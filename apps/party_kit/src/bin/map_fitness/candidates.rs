//! `map_fitness --candidates <game:map>` (docs/plans/active/arena-slices.md,
//! A5): arena-slice candidates for a map, by `slice_gen` on the loaded
//! map's bots' mesh, spawn spots and physics; a table of every kept
//! candidate's measures, pass or fail and why; for each one printed a
//! top-down picture (`safety::top_down`: the polygon, the spawn spots by how
//! seen they are, the spawn points, the wall over open floor in red) and its
//! `[[slice]]` entry ready to paste, in `<out>/candidates/<game>_<map>/`;
//! `--pick <n>` writes candidate n into `games/<game>/data/arena_slices.toml`
//! under `--repo` (an entry of the same map and id replaced). The name is
//! made from a compass word (where the slice lies on the map) and what it
//! holds (caves: a roof over 30% of its floor; a base: four spawn points of
//! the map or more; heights: 8 m of floors or more; else a hollow).
//!
//! Known shortcuts: the names don't read the map's named places (no map
//! the generator serves has any); `--remeasure` is not built (`--slice`
//! measures a slice as written).

use super::slice_gen::{Band, Candidate, CellIndex, Graph, Input, Settings, compass, generate};
use super::*;
use chimera_sim::minigame::map_fit::FitSettings;

/// The candidates' options.
#[derive(Clone, Debug, Default)]
pub(super) struct CandOpts {
    pub band: Option<Band>,
    pub top: usize,
    pub pick: Option<usize>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub repo: PathBuf,
}

pub(super) fn run(w: &mut World, o: &Opts, co: &CandOpts, c: &safety::SafetyInput) {
    let fit = FitSettings::shipped();
    let (game, map) = c.id.split_once(':').unwrap_or(("", c.id));
    let t0 = Instant::now();
    let mut graph = Graph::from_nav(c.nav, c.ids);
    let index = CellIndex::new(&graph);
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).expect("the spatial query");
    let filter = SpatialQueryFilter::from_mask(chimera_core::core::SOLID_LAYERS);
    let sees = chimera_sim::minigame::arena::sees_in(&q, &filter);
    let ray = |from: Vec3, dir: Vec3, max: f32| Dir3::new(dir).ok().and_then(|d| q.cast_ray(from, d, max, true, &filter)).map(|h| h.distance);
    // The seeds' bias: 8 level rays from each cell's eye, the share
    // meeting something within 6 m.
    let score: Vec<f32> = graph
        .cells
        .iter()
        .map(|cell| {
            let eye = cell.floor() + Vec3::Y * 1.6;
            (0..8)
                .filter(|k| {
                    let t = *k as f32 * std::f32::consts::TAU / 8.0;
                    ray(eye, Vec3::new(t.cos(), 0.0, t.sin()), 6.0).is_some()
                })
                .count() as f32
                / 8.0
        })
        .collect();
    // Growth's cost: open ground (8 level rays' mean length to 40 m, over
    // 40) up to 5 times a sheltered floor's.
    graph.cost = graph
        .cells
        .iter()
        .map(|cell| {
            let eye = cell.floor() + Vec3::Y * 1.6;
            let open: f32 = (0..8)
                .map(|k| {
                    let t = k as f32 * std::f32::consts::TAU / 8.0;
                    ray(eye, Vec3::new(t.cos(), 0.0, t.sin()), 40.0).unwrap_or(40.0)
                })
                .sum::<f32>()
                / (8.0 * 40.0);
            1.0 + 4.0 * open
        })
        .collect();
    // A roof over a cell (from 1 m over its floor, 5 m up): a cut there
    // costs ten times its width.
    graph.cut_price = graph.cells.iter().map(|cell| if ray(cell.floor() + Vec3::Y, Vec3::Y, 5.0).is_some() { 10.0 } else { 1.0 }).collect();
    // The spawn clusters' middles (points within 25 m of each other).
    let mut clusters: Vec<Vec<Vec3>> = Vec::new();
    for f in c.feet {
        match clusters.iter_mut().find(|cl| cl.iter().any(|p| p.distance(*f) < 25.0)) {
            Some(cl) => cl.push(*f),
            None => clusters.push(vec![*f]),
        }
    }
    let must: Vec<usize> = clusters
        .iter()
        .filter(|cl| cl.len() >= 3)
        .filter_map(|cl| {
            let mid = cl.iter().copied().sum::<Vec3>() / cl.len() as f32;
            (0..graph.len()).min_by(|a, b| graph.cells[*a].floor().distance(mid).total_cmp(&graph.cells[*b].floor().distance(mid)).then(a.cmp(b)))
        })
        .collect();
    let prep_s = t0.elapsed().as_secs_f32();
    let input = Input {
        graph: &graph,
        index: &index,
        spots: c.spots,
        points: c.feet,
        score: &score,
        must: &must,
        sight_m: c.sight_m,
        sees: &sees,
        ray: &ray,
    };
    let settings = Settings {
        table: o.table.min(800),
        ..Settings::default()
    };
    let bands: Vec<Band> = co.band.map_or(Band::ALL.to_vec(), |b| vec![b]);
    let file = co.repo.join("games").join(game).join("data").join("arena_slices.toml");
    let existing = fs::read_to_string(&file).unwrap_or_default();
    let map_name = existing_map_name(&existing, map).unwrap_or_else(|| chimera_sim::minigame::hud::map_display_name(c.id));
    let (lo, hi) = graph.cells.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), x| (l.min(x.min), h.max(x.max)));
    let dir = o.out.join("candidates").join(c.id.replace(':', "_"));
    let _ = fs::create_dir_all(&dir);
    let mut taken: Vec<String> = existing_names(&existing, map);
    let mut number = 0usize;
    let mut csv = String::from("n,band,id,name,passes,area_m2,sight_m,spots,spot_seen_by,floor_sees_point,see_pairs,seen_2,seen_4,seen_8,cover,long_sight_p95_m,point_floor_seen,point_seen_by,points,made,wall_m,open_m,indoor_m,stacked_m,corners,solidity,cut_area_m2,cut_m,suits,fails\n");
    let mut lines = vec![format!(
        "{}: arena-slice candidates ({} cells, {:.0} m2, {} spawn spots, {} spawn points in {} clusters of 3 or more; seeds' bias {prep_s:.1} s)",
        c.id,
        graph.len(),
        c.area_m2,
        c.spots.len(),
        c.feet.len(),
        must.len()
    )];
    let mut picked: Option<(Candidate, String, String)> = None;
    for band in bands {
        let t = Instant::now();
        let all = generate(&input, band, &settings, &fit, c.id);
        let gen_s = t.elapsed().as_secs_f32();
        let passing = all.iter().filter(|x| x.passes()).count();
        lines.push(format!(
            "--- band {} (aimed at {:.0} m2 at the map's sight {:.1} m): {} candidates after merging, {passing} pass ({gen_s:.1} s)",
            band.label(),
            band.target(c.sight_m, &fit),
            c.sight_m,
            all.len()
        ));
        lines.push(
            " n  name                      area   seen-by floor-sees see_pairs seen@2/4/8        cover long95 ptfloor ptseen spots open/wall m  corners solid suits  verdict"
                .into(),
        );
        for cand in all.iter().take(co.top) {
            number += 1;
            let centre = cand.poly.points().iter().copied().sum::<Vec2>() / cand.poly.points().len() as f32;
            let base = cand.points.len() >= 5;
            let roofed: f32 = graph.cells.iter().enumerate().filter(|(i, _)| cand.members[*i] && graph.cut_price[*i] > 1.0).map(|(_, x)| x.area()).sum();
            let what = if roofed > 0.3 * cand.report.area_m2 {
                "Caves"
            } else if base {
                "Base"
            } else if cand.vertical_m > 8.0 {
                "Heights"
            } else {
                "Hollow"
            };
            let mut name = format!("{} {what}", compass(centre, lo, hi));
            if name.starts_with("Centre ") {
                name = format!("{what} at the Centre");
            }
            let mut k = 2;
            let stem = name.clone();
            while taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
                name = format!("{stem} {k}");
                k += 1;
            }
            taken.push(name.clone());
            let id = name.to_ascii_lowercase().replace(' ', "_");
            let r = &cand.report;
            let s = &r.safety;
            let suits = cand.verdict.players.map_or("-".into(), |[a, b]| format!("{a}-{b}"));
            lines.push(format!(
                "{number:>2}  {:<24} {:>6.0} {:>7.1}% {:>9.1}% {:>9.3} {:.2}/{:.2}/{:.2}  {:>5.2} {:>5.0} {:>7.2} {:>6.2} {:>5} {:>4.0}/{:<5.0} {:>7} {:>5.2} {:<6} {}",
                format!("{map_name}: {name}").chars().take(24).collect::<String>(),
                r.area_m2,
                100.0 * s.spot_seen_by,
                100.0 * r.floor_sees_point,
                s.see_pairs,
                s.seen_at(2),
                s.seen_at(4),
                s.seen_at(8),
                r.cover,
                r.long_sight_p95_m,
                r.point_floor_seeing(),
                r.point_seen_by(),
                s.spots,
                cand.wall.open_m,
                cand.wall.open_m + cand.wall.along_m,
                cand.poly.points().len(),
                cand.solidity,
                suits,
                if cand.passes() { "PASS".to_string() } else { format!("FAIL: {}", cand.fails.join("; ")) }
            ));
            csv.push_str(&format!(
                "{number},{},{id},{name},{},{:.0},{:.1},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.1},{:.3},{:.3},{},{},{:.0},{:.0},{:.1},{:.1},{},{:.2},{:.0},{:.1},{suits},{}\n",
                band.label(),
                cand.passes(),
                r.area_m2,
                r.sight_m,
                s.spots,
                s.spot_seen_by,
                r.floor_sees_point,
                s.see_pairs,
                s.seen_at(2),
                s.seen_at(4),
                s.seen_at(8),
                r.cover,
                r.long_sight_p95_m,
                r.point_floor_seeing(),
                r.point_seen_by(),
                cand.points.len(),
                cand.made.len(),
                cand.wall.open_m + cand.wall.along_m,
                cand.wall.open_m,
                cand.wall.indoor_m,
                cand.wall.stacked_m,
                cand.poly.points().len(),
                cand.solidity,
                cand.cut_area_m2,
                cand.cut_m,
                cand.fails.join("; ").replace(',', " ")
            ));
            let entry = entry_text(map, &id, &name, &map_name, cand, number, c.id);
            let stem = format!("{number:02}_{}_{id}", band.label());
            let _ = fs::write(dir.join(format!("{stem}.toml")), &entry);
            let spots: Vec<Vec3> = c.spots.iter().copied().filter(|p| cand.poly.contains3(*p)).collect();
            let ci = safety::SafetyInput {
                id: c.id,
                spots: &spots,
                feet: &cand.points,
                nav: c.nav,
                ids: c.ids,
                area_m2: r.area_m2,
                sight_m: r.sight_m,
                game_units: c.game_units,
            };
            safety::top_down(&q, &dir.join(format!("{stem}.png")), &ci, &cand.table, Some(cand.poly), &cand.made, &[], Some(cand.seed), &cand.wall.open_at, &cand.wall.roofed_at);
            if co.pick == Some(number) {
                picked = Some((cand.clone(), co.id.clone().unwrap_or(id.clone()), co.name.clone().unwrap_or(name.clone())));
            }
        }
    }
    lines.push(format!("total {:.1} s; pictures and entries in {}", t0.elapsed().as_secs_f32(), dir.display()));
    let text = lines.join("\n");
    println!("{text}");
    let _ = fs::write(dir.join("candidates.txt"), &text);
    let _ = fs::write(dir.join("candidates.csv"), &csv);
    match (co.pick, picked) {
        (Some(n), None) => eprintln!("--pick {n}: no candidate numbered {n} was printed"),
        (Some(n), Some((cand, id, name))) => {
            let entry = entry_text(map, &id, &name, &map_name, &cand, n, c.id);
            match write_pick(&file, game, map, &id, &existing, &entry) {
                Ok(()) => println!("picked candidate {n} as {map}@{id} (\"{map_name}: {name}\") into {}", file.display()),
                Err(e) => eprintln!("--pick {n}: {e}"),
            }
        }
        _ => {}
    }
}

/// The `map_name` an existing entry of `map` gives.
fn existing_map_name(text: &str, map: &str) -> Option<String> {
    blocks(text).into_iter().filter(|b| field(b, "map").as_deref() == Some(map)).find_map(|b| field(&b, "map_name"))
}

/// The names of `map`'s existing slices.
fn existing_names(text: &str, map: &str) -> Vec<String> {
    blocks(text).into_iter().filter(|b| field(b, "map").as_deref() == Some(map)).filter_map(|b| field(&b, "name")).collect()
}

/// A `key = "value"` line's value in an entry's text.
fn field(block: &str, key: &str) -> Option<String> {
    block.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// The file's `[[slice]]` entries, each with the comment lines right above
/// it.
fn blocks(text: &str) -> Vec<String> {
    spans(text).into_iter().map(|(a, b)| text.lines().skip(a).take(b - a).collect::<Vec<_>>().join("\n")).collect()
}

/// Line ranges of `blocks`.
fn spans(text: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let heads: Vec<usize> = (0..lines.len()).filter(|i| lines[*i].trim() == "[[slice]]").collect();
    let mut out = Vec::new();
    for (k, h) in heads.iter().enumerate() {
        let mut start = *h;
        while start > 0 && lines[start - 1].trim_start().starts_with('#') {
            start -= 1;
        }
        let end = heads.get(k + 1).map_or(lines.len(), |next| {
            let mut e = *next;
            while e > 0 && lines[e - 1].trim_start().starts_with('#') {
                e -= 1;
            }
            e
        });
        out.push((start, end));
    }
    out
}

/// The candidate's `[[slice]]` entry, its measures in the comment above.
fn entry_text(map: &str, id: &str, name: &str, map_name: &str, c: &Candidate, n: usize, key: &str) -> String {
    let r = &c.report;
    let s = &r.safety;
    let pts: Vec<String> = c.poly.points().iter().map(|p| format!("[{:.1}, {:.1}]", p.x, p.y)).collect();
    let mut t = format!(
        "# {map_name}: {name}. Generated by `map_fitness --candidates {key} --band {}` (candidate {n}),\n\
         # measured as a candidate: a spawn spot is seen by {:.1}% of the other spots, {:.1}% of the floor\n\
         # sees a spawn point; see_pairs {:.3}, seen_at_spawn 2/4/8/12/16 {}, cover {:.2}, long sight p95\n\
         # {:.0} m, point floor seen {:.2}, point seen by {:.2}; {} of the map's spawn points inside with the\n\
         # seed's, {} made; wall {:.0} m, {:.0} m of it over open floor. {}\n",
        c.band.label(),
        100.0 * s.spot_seen_by,
        100.0 * r.floor_sees_point,
        s.see_pairs,
        s.seen_at_spawn.iter().map(|(_, v)| format!("{v:.2}")).collect::<Vec<_>>().join("/"),
        r.cover,
        r.long_sight_p95_m,
        r.point_floor_seeing(),
        r.point_seen_by(),
        c.points.len(),
        c.made.len(),
        c.wall.open_m + c.wall.along_m,
        c.wall.open_m,
        if c.passes() { "Passes [slice].".to_string() } else { format!("FAILS: {}.", c.fails.join("; ")) }
    );
    t.push_str(&format!(
        "[[slice]]\nmap = \"{map}\"\nid = \"{id}\"\nname = \"{name}\"\nmap_name = \"{map_name}\"\npoints = [{}]\nseed = [{:.1}, {:.2}, {:.1}]\n",
        pts.join(", "),
        c.seed.x,
        c.seed.y,
        c.seed.z
    ));
    t.push_str(&measure_lines(r, c.verdict.players));
    t
}

/// The keys `measure_lines` writes (an entry's measured fields, which the
/// pool reads: `arena::Entry`).
const MEASURED: [&str; 10] = [
    "players",
    "area_m2",
    "sight_m",
    "spots",
    "see_pairs",
    "seen_at_spawn",
    "cover",
    "long_sight_p95_m",
    "point_floor_seen",
    "point_seen_by",
];

/// A report's measures as an entry's lines (the players it suits, when
/// it passes).
pub(super) fn measure_lines(r: &chimera_sim::minigame::slice_safety::Report, players: Option<[usize; 2]>) -> String {
    let s = &r.safety;
    let mut t = String::new();
    if let Some([a, b]) = players {
        t.push_str(&format!("players = [{a}, {b}]\n"));
    }
    t.push_str(&format!(
        "area_m2 = {:.0}\nsight_m = {:.1}\nspots = {}\nsee_pairs = {:.3}\nseen_at_spawn = {{ {} }}\ncover = {:.3}\nlong_sight_p95_m = {:.1}\npoint_floor_seen = {:.3}\npoint_seen_by = {:.3}\n",
        r.area_m2,
        r.sight_m,
        s.spots,
        s.see_pairs,
        s.seen_at_spawn.iter().map(|(n, v)| format!("{n} = {v:.3}")).collect::<Vec<_>>().join(", "),
        r.cover,
        r.long_sight_p95_m,
        r.point_floor_seeing(),
        r.point_seen_by()
    ));
    t
}

/// `--slice <key> --remeasure`: the measured fields of the slice's entry in
/// `file` replaced by `measures` (`measure_lines`), checked by the game's
/// reader first.
pub(super) fn remeasure(file: &std::path::Path, game: &str, map: &str, id: &str, measures: &str) -> Result<(), String> {
    let existing = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let lines: Vec<&str> = existing.lines().collect();
    let (a, b) = spans(&existing)
        .into_iter()
        .find(|(a, b)| {
            let block = lines[*a..*b].join("\n");
            field(&block, "map").as_deref() == Some(map) && field(&block, "id").as_deref() == Some(id)
        })
        .ok_or_else(|| format!("no entry {map}@{id} in {}", file.display()))?;
    let mut block: Vec<String> = lines[a..b]
        .iter()
        .filter(|l| !l.split_once('=').is_some_and(|(k, _)| MEASURED.contains(&k.trim())))
        .map(|l| l.to_string())
        .collect();
    while block.last().is_some_and(|l| l.trim().is_empty()) {
        block.pop();
    }
    block.extend(measures.trim_end().lines().map(str::to_string));
    let entry = block.join("\n");
    write_pick(file, game, map, id, &existing, &entry)
}

/// `entry` into the game's slices file (`existing` its text): replacing the
/// entry of the same map and id, else added at the end; checked by the
/// game's reader first.
fn write_pick(file: &std::path::Path, game: &str, map: &str, id: &str, existing: &str, entry: &str) -> Result<(), String> {
    let lines: Vec<&str> = existing.lines().collect();
    let found = spans(existing).into_iter().find(|(a, b)| {
        let block = lines[*a..*b].join("\n");
        field(&block, "map").as_deref() == Some(map) && field(&block, "id").as_deref() == Some(id)
    });
    let mut out = match found {
        Some((a, b)) => {
            let mut v: Vec<String> = lines[..a].iter().map(|s| s.to_string()).collect();
            v.extend(entry.trim_end().lines().map(str::to_string));
            if b < lines.len() {
                v.push(String::new());
                v.extend(lines[b..].iter().map(|s| s.to_string()));
            }
            v.join("\n")
        }
        None => format!("{}\n\n{}", existing.trim_end(), entry.trim_end()),
    };
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.push('\n');
    chimera_sim::minigame::arena::parse(game, &out)?;
    fs::write(file, out).map_err(|e| format!("{}: {e}", file.display()))
}
