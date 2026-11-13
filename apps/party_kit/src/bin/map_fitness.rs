//! Development tool: how fit each installed map is for a gun game's pool
//! (docs/plans/active/map-pool-fitness.md). Loads every map of the party's
//! games (CS:S, Combat Arms, HL2:DM: each install's `GameInfo::map_names`)
//! headless, one at a time, as the dedicated server does (no textures, no
//! kit), on Combat Arms' simulation (`CaSim`) with the party's parts
//! (`PartyPartsPlugin`: the map logic's brush entities) and the gun game
//! started; floods the bots' navigation
//! mesh from the spawn points (Combat Arms' walker, `bots::brain::
//! nav_params`, jump-ups included, meeting the player clips; bounded to the clusters the spawn
//! spots' walking player, `spawns::walker`, reaches where the map has
//! visibility data, `nav_gen::Bounds`) and
//! measures it:
//!
//! - `area_m2`: the walkable area of the mesh's main region (areas walked
//!   to from a spawn point and back, as the spawn spots take them):
//!   disconnected scenery left out. `mesh_m2`: the whole flood;
//!   `spot_area_m2`: the main region of a walking player's mesh, the one
//!   the spawn spots are drawn on (`spawns::walker`).
//! - `pair_p50_s`, `pair_p90_s`: shortest walking paths between two
//!   random points of that area (area-weighted samples), over the run
//!   speed. `spawn_pair_*`: the same between the map's spawn points;
//!   `spawn_hub_*`: from the spawn points to the hub, the area whose mean
//!   walking distance to all of the area is least (the place every
//!   crossing passes nearest: the most visited, as a model).
//! - `meet_N_s`: with N players spread over the area at random (as
//!   `random_hidden` spawns them), the median over players of the walk to
//!   the nearest other player, over the run speed (Monte Carlo over the
//!   samples), for N = 2, 4, 8, 12, 16.
//! - `sight_m`: the mean length of 16 level rays from eye height at each
//!   sample (capped at 50 m). `wander_N_s`: N players wandering at random
//!   meet as a gas's molecules collide: area / ((N - 1) * 2 * sight *
//!   4/pi * speed), the mean time to a player's next meeting.
//! - `spawns`, `spawns_in_region`, `spawn_min_m`, `spawn_median_m`: the
//!   map's spawn points, those on the spots' main region (below), and the least and
//!   median straight-line distance between two of them.
//! - Spawn spots off the map (`party_kit::spot_checks`, what the heavy
//!   guard `spawn_spots` runs on the pool): `spots` is what
//!   `random_hidden` picks from (`spawns::candidates`; none: the spawn
//!   points); `spots_off` those off the main region (`off_region`: the
//!   navigation file's where the map has one, else the bots' mesh's as
//!   `spot_checks::bots_mesh` makes it),
//!   off the map's own navigation file (`off_nav_file`, Source's
//!   `maps/<name>.nav` when it has one, `map::MapFloors`: `has_nav_file`;
//!   1.5 m and 2 m of slack) or outside the playable box of a map with a
//!   3D skybox (`off_box`), and `spots_off_at` a few of them in the map's
//!   own units (Source: x y z in inches).
//! - `stuck`: spots where a standing player's box (Source's 32 x 72
//!   units, `spawns::standing_box`), from a step (18 units) over the feet
//!   to the head, meets what characters collide with (a wall, a prop, a
//!   ceiling, a player clip: not a slope or a stair under its corners),
//!   and `stuck_at` a few, with what.
//! - `--probe x,y,z` (a bug report's `setpos`): what the box there meets
//!   (the player-solid brushes too), the spots and spawn points near it,
//!   and after five seconds of the map the floor, the ceiling and
//!   everything within 4 m. `--leaks`: where the spots' walk leaves the
//!   navigation file, and where the file or the bots' walk goes on and
//!   it doesn't. `--dump`: the spots, the file's and the bots' floors, a
//!   top-down height map and the map's entities as files. `--bare`:
//!   without the map logic; `--client`: loaded as a client loads it.
//!
//! Paths are Dijkstra over the mesh's areas, centre to centre (an area is
//! at most 4 m a side, so paths come out a few percent long). One run
//! speed for every game (`--speed`, default CS:S's 250 units/s, 6.35 m/s),
//! so maps compare across games. Writes `fitness.csv` to `--out` (default
//! `target/map_fitness`), a row per map as it is measured; `--resume`
//! skips maps already in it. `--write <repo>` then writes each game's
//! `games/<game>/data/map_fitness.toml` from that CSV (the measurements
//! an automatic gun game's pool draws by: `chimera_sim::minigame::
//! map_fit`; per map its area, sight, spawn points and spots, and the
//! wander times for reading); `--csv <file> --write <repo>` writes them
//! from a sweep's CSV without loading anything. Write from a whole sweep:
//! a map the file lacks is in the pool at a middle weight. Waits while the machine has less than
//! `--min-free-gb` (default 8) of memory available (Linux).
//!
//! Known shortcuts: no ladders or crouching (the generated flood's);
//! sight is level rays from area centres, no field of view or fog; the
//! wander model assumes players spread evenly and walk blind; a map whose
//! flood leaks outside the playable space shows a large area (`capped` at
//! the flood's cell limit), and the off-map check needs a navigation file
//! or a 3D skybox to see it.

use std::{
    collections::{BTreeSet, BinaryHeap},
    fs,
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    process::ExitCode,
    time::Instant,
};

use avian3d::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{core::SpawnPoint, game_registry::GameInfo};
use chimera_world::{
    harness::Sim,
    map::{
        KitLoad, LoadContext, MapLoadOptions, MapSimPlugin, PlayableArea, SPAWN_LIFT,
        nav::{NavMesh, Via},
        nav_gen,
        vis::ActiveVisibility,
    },
};
use combat_arms::harness::CaSim;

const USAGE: &str = "\
usage: map_fitness [options]
  --game <ids>       comma-separated: cs_source, combat_arms, hl2dm (default: all three)
  --filter <text>    only maps whose name contains <text> (several: comma-separated)
  --list             only list each game's installed maps
  --out <dir>        report folder (default: target/map_fitness)
  --resume           skip maps already in <out>/fitness.csv
  --speed <m/s>      run speed for times (default 6.35: CS:S's 250 units/s)
  --probe <x,y,z>    also say what a standing player's box at this point meets (the map's own
                     units: Source's setpos; repeatable)
  --leaks            also list where the spawn spots' walk leaves the map's navigation file (the
                     links from an area on it to one off it, in the map's units)
  --dump             also write each map's spawn spots to <out>/<game>_<map>_spots.csv (x, y, z
                     in the map's units, and whether each is off the region, the navigation
                     file or stuck)
  --bare             without the party's parts (no map logic): the map as loaded
  --client           load the map as a client does (textures, the players' kit), not as the
                     dedicated server
  --samples <n>      area samples for the path statistics (default 256)
  --min-free-gb <n>  wait while less memory than this is available (default 8)
  --write <repo>     then write games/<game>/data/map_fitness.toml under <repo> from the CSV
  --csv <file>       with --write: write from this sweep's CSV, measuring nothing";

const N_PLAYERS: [usize; 5] = [2, 4, 8, 12, 16];
const HEADER: &str = "game,map,loads,error,load_s,nav_s,spawns,spawns_in_region,own_nav,spots,spots_off,has_nav_file,off_region,off_nav_file,off_box,spots_off_at,stuck,stuck_at,spawn_min_m,spawn_median_m,\
area_m2,mesh_m2,spot_area_m2,areas,capped,pair_p50_s,pair_p90_s,spawn_pair_p50_s,spawn_pair_p90_s,spawn_hub_p50_s,spawn_hub_p90_s,\
meet_2_s,meet_4_s,meet_8_s,meet_12_s,meet_16_s,sight_m,wander_4_s,wander_8_s,wander_12_s";
const WANDER_PLAYERS: [usize; 3] = [4, 8, 12];
/// A standing CS:S player's box: 32 units wide, 72 tall (m).
const PLAYER_W: f32 = 32.0 * 0.0254;
const PLAYER_H: f32 = 72.0 * 0.0254;

/// The box is tested from a step up (18 units: a slope or a stair under
/// its corners is walked off, not stuck in) to the head.
const STEP: f32 = 18.0 * 0.0254;
const BOX_CENTRE: f32 = STEP + (PLAYER_H - STEP) / 2.0;

fn player_box() -> Collider {
    Collider::cuboid(PLAYER_W, PLAYER_H - STEP - 0.05, PLAYER_W)
}

/// The longest sight ray (m).
const SIGHT_CAP: f32 = 50.0;

struct Opts {
    games: Vec<String>,
    filter: Vec<String>,
    list: bool,
    out: PathBuf,
    resume: bool,
    speed: f32,
    samples: usize,
    probes: Vec<Vec3>,
    leaks: bool,
    bare: bool,
    client: bool,
    dump: bool,
    min_free_gb: f64,
    write: Option<PathBuf>,
    csv: Option<PathBuf>,
}

fn parse() -> Result<Opts, String> {
    let mut o = Opts {
        games: vec!["cs_source".into(), "combat_arms".into(), "hl2dm".into()],
        filter: Vec::new(),
        list: false,
        out: PathBuf::from("target/map_fitness"),
        resume: false,
        speed: 6.35,
        samples: 256,
        probes: Vec::new(),
        min_free_gb: 8.0,
        leaks: false,
        bare: false,
        client: false,
        dump: false,
        write: None,
        csv: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--game" => o.games = val()?.split(',').map(str::to_string).collect(),
            "--filter" => o.filter = val()?.split(',').map(str::to_string).collect(),
            "--list" => o.list = true,
            "--out" => o.out = PathBuf::from(val()?),
            "--resume" => o.resume = true,
            "--speed" => o.speed = val()?.parse().map_err(|e| format!("--speed: {e}"))?,
            "--probe" => {
                let v: Vec<f32> = val()?.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let [x, y, z] = v[..] else {
                    return Err("--probe x,y,z".into());
                };
                // Source units (z up); a Combat Arms map's are metres, y up.
                o.probes.push(Vec3::new(x, z, -y) * 0.0254);
            }
            "--leaks" => o.leaks = true,
            "--bare" => o.bare = true,
            "--client" => o.client = true,
            "--dump" => o.dump = true,
            "--samples" => o.samples = val()?.parse().map_err(|e| format!("--samples: {e}"))?,
            "--min-free-gb" => o.min_free_gb = val()?.parse().map_err(|e| format!("--min-free-gb: {e}"))?,
            "--write" => o.write = Some(PathBuf::from(val()?)),
            "--csv" => o.csv = Some(PathBuf::from(val()?)),
            "-h" | "--help" => return Err(String::new()),
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(o)
}

fn info(game: &str) -> Option<&'static GameInfo> {
    match game {
        "cs_source" => Some(&cs_source::game::INFO),
        "combat_arms" => Some(&combat_arms::game::INFO),
        "hl2dm" => Some(&hl2dm::game::INFO),
        _ => None,
    }
}

/// Memory available, GB (None where /proc/meminfo isn't).
fn available_gb() -> Option<f64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0 / 1024.0)
}

fn wait_for_memory(min: f64) {
    let mut said = false;
    while let Some(gb) = available_gb().filter(|gb| *gb < min) {
        if !said {
            eprintln!("map_fitness: {gb:.1} GB available, waiting for {min} GB");
            said = true;
        }
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

fn main() -> ExitCode {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(csv) = &o.csv {
        let Some(repo) = &o.write else {
            eprintln!("--csv needs --write <repo>");
            return ExitCode::FAILURE;
        };
        return write_data(csv, repo, &o.games, o.speed);
    }
    let mut maps: Vec<(String, String)> = Vec::new();
    let mut mounts: Vec<(String, chimera_core::mount::Mount)> = Vec::new();
    for game in &o.games {
        let Some(info) = info(game) else {
            eprintln!("unknown game {game}");
            return ExitCode::FAILURE;
        };
        let Some(mount) = info.mount() else {
            eprintln!("{game}: no install mounted; skipped");
            continue;
        };
        let names = (info.map_names)(&mount);
        mounts.push((game.clone(), mount));
        eprintln!("{game}: {} maps installed", names.len());
        for name in names {
            let lower = name.to_ascii_lowercase();
            if o.filter.is_empty() || o.filter.iter().any(|f| lower.contains(f.as_str())) {
                maps.push((game.clone(), lower));
            }
        }
    }
    if o.list {
        for (g, m) in &maps {
            println!("{g}:{m}");
        }
        return ExitCode::SUCCESS;
    }
    if let Err(e) = fs::create_dir_all(&o.out) {
        eprintln!("{}: {e}", o.out.display());
        return ExitCode::FAILURE;
    }
    let csv = o.out.join("fitness.csv");
    let done: BTreeSet<String> = if o.resume {
        fs::read_to_string(&csv)
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let mut f = l.split(',');
                Some(format!("{}:{}", f.next()?, f.next()?))
            })
            .collect()
    } else {
        BTreeSet::new()
    };
    let fresh = !o.resume || !csv.exists();
    let mut file = match fs::OpenOptions::new()
        .create(true)
        .append(o.resume)
        .write(true)
        .truncate(!o.resume)
        .open(&csv)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", csv.display());
            return ExitCode::FAILURE;
        }
    };
    if fresh {
        let _ = writeln!(file, "{HEADER}");
    }
    let context = LoadContext::default();
    let total = maps.len();
    for (i, (game, name)) in maps.iter().enumerate() {
        let id = format!("{game}:{name}");
        if done.contains(&id) {
            continue;
        }
        wait_for_memory(o.min_free_gb);
        eprintln!("[{}/{total}] {id}", i + 1);
        let row = match catch_unwind(AssertUnwindSafe(|| {
            measure(
                &id,
                &context,
                &o,
                mounts.iter().find(|(g, _)| g == game).map(|(_, m)| m),
            )
        })) {
            Ok(r) => r,
            Err(p) => {
                let why = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panic".into());
                Row::failed(format!("panic: {why}"), 0.0)
            }
        };
        let line = row.csv(game, name);
        eprintln!("  {line}");
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
    drop(file);
    if let Some(repo) = &o.write {
        return write_data(&csv, repo, &o.games, o.speed);
    }
    ExitCode::SUCCESS
}

/// Each of `games`' `games/<game>/data/map_fitness.toml` under `repo`
/// from the sweep's CSV `csv`.
fn write_data(csv: &std::path::Path, repo: &std::path::Path, games: &[String], speed: f32) -> ExitCode {
    let text = match fs::read_to_string(csv) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {e}", csv.display());
            return ExitCode::FAILURE;
        }
    };
    for game in games {
        let doc = fitness_toml(&text, game, speed);
        let path = repo.join("games").join(game).join("data").join("map_fitness.toml");
        if let Err(e) = fs::write(&path, &doc.0) {
            eprintln!("{}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!("{}: {} maps", path.display(), doc.1);
    }
    ExitCode::SUCCESS
}

/// `game`'s rows of a sweep's CSV as its data/map_fitness.toml, and how
/// many maps.
fn fitness_toml(csv: &str, game: &str, speed: f32) -> (String, usize) {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
    let col = |row: &[&str], name: &str| -> String {
        header.iter().position(|h| *h == name).and_then(|i| row.get(i)).map_or(String::new(), |v| v.to_string())
    };
    let num = |v: String| -> f32 { v.parse().unwrap_or(0.0) };
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in lines {
        let row: Vec<&str> = line.split(',').collect();
        if col(&row, "game") != game {
            continue;
        }
        let name = col(&row, "map").to_ascii_lowercase();
        let area = num(col(&row, "area_m2")).max(0.0);
        let loads = col(&row, "loads") == "true" && area > 0.0;
        let wander: Vec<String> = WANDER_PLAYERS
            .iter()
            .map(|n| format!("{n} = {:.1}", num(col(&row, &format!("wander_{n}_s")))))
            .collect();
        let mut t = format!("[maps.{name}]\n");
        if !loads {
            t.push_str("loads = false\n");
        }
        t.push_str(&format!(
            "area_m2 = {area:.0}\nsight_m = {:.1}\nspot_area_m2 = {:.0}\nspawns = {}\nspots = {}\nspots_off = {}\nstuck = {}\nwander_s = {{ {} }}\n",
            num(col(&row, "sight_m")),
            num(col(&row, "spot_area_m2")).max(0.0),
            num(col(&row, "spawns")) as u32,
            num(col(&row, "spots")) as u32,
            num(col(&row, "spots_off")) as u32,
            num(col(&row, "stuck")) as u32,
            wander.join(", ")
        ));
        rows.push((name, t));
    }
    rows.sort();
    rows.dedup_by(|a, b| a.0 == b.0);
    let mut out = format!(
        "# {game}'s maps measured for an automatic gun game's pool, generated by the\n\
         # map_fitness tool (`--write`; docs/plans/active/map-pool-fitness.md): edit\n\
         # data/map_pool.toml instead. Per map: its walkable area (m2), mean sight (m),\n\
         # the area its spawn spots cover, its spawn points, spawn spots, those off\n\
         # the map and those a player is stuck in, and its wander times at 4/8/12\n\
         # players (s; read by chimera_sim::minigame::map_fit, which works them out\n\
         # again from area and sight at any count).\n\
         speed_mps = {speed}\n"
    );
    let n = rows.len();
    for (_, t) in rows {
        out.push('\n');
        out.push_str(&t);
    }
    (out, n)
}

#[derive(Default)]
struct Row {
    error: Option<String>,
    load_s: f32,
    nav_s: f32,
    spawns: usize,
    spawns_in_region: usize,
    own_nav: bool,
    spots: usize,
    spots_off: usize,
    has_nav_file: bool,
    off_region: usize,
    off_nav_file: usize,
    off_box: usize,
    spots_off_at: String,
    stuck: usize,
    stuck_at: String,
    spawn_min_m: f32,
    spawn_median_m: f32,
    area_m2: f32,
    mesh_m2: f32,
    spot_area_m2: f32,
    areas: usize,
    capped: bool,
    pair: (f32, f32),
    spawn_pair: (f32, f32),
    spawn_hub: (f32, f32),
    meet: [f32; N_PLAYERS.len()],
    sight_m: f32,
    wander: [f32; WANDER_PLAYERS.len()],
}

impl Row {
    fn failed(error: String, load_s: f32) -> Self {
        Self {
            error: Some(error),
            load_s,
            ..Default::default()
        }
    }

    fn csv(&self, game: &str, map: &str) -> String {
        let f = |x: f32| {
            if x.is_finite() {
                format!("{x:.1}")
            } else {
                String::new()
            }
        };
        let err = self.error.as_deref().unwrap_or("").replace([',', '\n', '"'], " ");
        let mut cols = vec![
            game.to_string(),
            map.to_string(),
            (self.error.is_none() || self.spawns > 0).to_string(),
            err.chars().take(160).collect(),
            f(self.load_s),
            f(self.nav_s),
            self.spawns.to_string(),
            self.spawns_in_region.to_string(),
            self.own_nav.to_string(),
            self.spots.to_string(),
            self.spots_off.to_string(),
            self.has_nav_file.to_string(),
            self.off_region.to_string(),
            self.off_nav_file.to_string(),
            self.off_box.to_string(),
            self.spots_off_at.replace(',', " "),
            self.stuck.to_string(),
            self.stuck_at.replace(',', " "),
            f(self.spawn_min_m),
            f(self.spawn_median_m),
            format!("{:.0}", self.area_m2),
            format!("{:.0}", self.mesh_m2),
            format!("{:.0}", self.spot_area_m2),
            self.areas.to_string(),
            self.capped.to_string(),
            f(self.pair.0),
            f(self.pair.1),
            f(self.spawn_pair.0),
            f(self.spawn_pair.1),
            f(self.spawn_hub.0),
            f(self.spawn_hub.1),
        ];
        cols.extend(self.meet.iter().map(|m| f(*m)));
        cols.push(f(self.sight_m));
        cols.extend(self.wander.iter().map(|m| f(*m)));
        cols.join(",")
    }
}

fn measure(id: &str, context: &LoadContext, o: &Opts, _mount: Option<&chimera_core::mount::Mount>) -> Row {
    let t = Instant::now();
    let options = if o.client {
        MapLoadOptions::default().in_context(context)
    } else {
        MapLoadOptions {
            kit: KitLoad::Only(&[]),
            ..MapLoadOptions::lean().in_context(context)
        }
    };
    let data = match party_kit::load_map_with(id, &options) {
        Ok(d) => d,
        Err(e) => return Row::failed(e, t.elapsed().as_secs_f32()),
    };
    // The party's parts: the map logic's brush entities (doors, func_brush,
    // breakables) and the gun game, as the apps run it.
    let mut sim = if o.bare {
        Sim::new(CaSim, MapSimPlugin::new(data))
    } else {
        Sim::new(CaSim, (MapSimPlugin::new(data), party_kit::PartyPartsPlugin))
    };
    let _ = chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME);
    sim.ticks(3);
    let load_s = t.elapsed().as_secs_f32();
    let w = sim.app.world_mut();
    let mut feet: Vec<Vec3> = w
        .query_filtered::<&Transform, With<SpawnPoint>>()
        .iter(w)
        .map(|t| t.translation - Vec3::Y * SPAWN_LIFT)
        .collect();
    feet.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
    // What `random_hidden` picks from: the spawn spots (the map's own
    // mesh, else a walking player's generated one); none: the spawn points.
    let own_nav = w.get_resource::<NavMesh>().is_some_and(|n| !n.generated);
    // The map's own navigation file (Source's `maps/<name>.nav`, its
    // author's or its bots' generator's, `MapFloors`): the playable
    // floors, for the off-map check.
    let file_nav = party_kit::spot_checks::file_nav(w);
    let playable = w.get_resource::<PlayableArea>().map(|p| p.0);
    let candidates = chimera_sim::minigame::spawns::candidates(w);
    // Stuck: a standing CS:S player's box (32 x 72 units) at the spot, a
    // few centimetres up, meets what a character's movement meets (the
    // player clips' collider among it).
    let (spot_walker, spot_bound) = chimera_sim::minigame::spawns::walker_for(w);
    let (stuck, probes) = {
        let used: &[Vec3] = if candidates.is_empty() { &feet } else { &candidates };
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).expect("the spatial query");
        let hits = |at: Vec3| party_kit::spot_checks::box_meets(&q, at);
        let stuck: Vec<(Vec3, Vec<Entity>)> = used
            .iter()
            .filter_map(|f| Some((*f, hits(*f))).filter(|(_, h)| !h.is_empty()))
            .collect();
        let probes: Vec<(Vec3, Vec<Entity>)> = o.probes.iter().map(|f| (*f, hits(*f))).collect();
        (stuck, probes)
    };
    let describe = |w: &World, e: Entity| {
        let name = |e: Entity| w.get::<Name>(e).map(|n| n.to_string());
        let parent = w.get::<ChildOf>(e).and_then(|c| name(c.parent()));
        match (name(e), parent) {
            (Some(n), Some(p)) => format!("{n} / {p}"),
            (Some(n), None) => n,
            (None, Some(p)) => format!("? / {p}"),
            (None, None) => format!("{e}"),
        }
    };
    let source = !id.starts_with("combat_arms:");
    let at = move |q: Vec3| {
        if source {
            format!("{:.0} {:.0} {:.0}", q.x / 0.0254, -q.z / 0.0254, q.y / 0.0254)
        } else {
            format!("{:.1} {:.1} {:.1} m", q.x, q.y, q.z)
        }
    };
    for (p, hits) in &probes {
        let what: Vec<String> = hits.iter().map(|e| describe(w, *e)).collect();
        let near = used_spots(&candidates, &feet)
            .iter()
            .map(|c| c.distance(*p))
            .fold(f32::INFINITY, f32::min);
        let point = feet.iter().map(|c| c.distance(*p)).fold(f32::INFINITY, f32::min);
        let mut close: Vec<Vec3> = used_spots(&candidates, &feet)
            .iter()
            .copied()
            .filter(|c| c.xz().distance(p.xz()) < 3.0)
            .collect();
        close.sort_by(|a, b| a.distance(*p).total_cmp(&b.distance(*p)));
        for c in close.iter().take(4) {
            let stuck_in = stuck
                .iter()
                .find(|(s, _)| s == c)
                .map(|(_, h)| h.iter().map(|e| describe(w, *e)).collect::<Vec<_>>().join(" + "));
            eprintln!(
                "    spot {} ({:.2} m): box meets {}",
                at(*c),
                c.distance(*p),
                stuck_in.unwrap_or_else(|| "nothing".into())
            );
        }
        eprintln!(
            "  probe {}: box meets {}; nearest spawn spot {near:.2} m, nearest spawn point {point:.2} m",
            at(*p),
            if what.is_empty() {
                "nothing".into()
            } else {
                what.join("; ")
            }
        );
    }
    let stuck_count = stuck.len();
    let stuck_spots: BTreeSet<[u32; 3]> = stuck.iter().map(|(p, _)| p.to_array().map(f32::to_bits)).collect();
    let stuck_at = stuck
        .iter()
        .step_by((stuck.len() / 6).max(1))
        .take(6)
        .map(|(p, h)| {
            format!(
                "{} in {}",
                at(*p),
                h.iter().map(|e| describe(w, *e)).collect::<Vec<_>>().join(" + ")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    if feet.is_empty() {
        return Row::failed("no spawn points".into(), load_s);
    }
    let mut row = Row {
        load_s,
        spawns: feet.len(),
        ..Default::default()
    };
    let mut straight: Vec<f32> = Vec::new();
    for i in 0..feet.len() {
        for j in i + 1..feet.len() {
            straight.push(feet[i].distance(feet[j]));
        }
    }
    straight.sort_by(f32::total_cmp);
    row.spawn_min_m = straight.first().copied().unwrap_or(f32::NAN);
    row.spawn_median_m = pct(&straight, 0.5);

    let t = Instant::now();
    // The bots' walker (Combat Arms', jump-ups included), bounded to the
    // playable space where the map has visibility data and meeting the
    // player clips: the space players play.
    let p = combat_arms::game::bots::brain::nav_params();
    let vis = w.get_resource::<ActiveVisibility>().map(|v| v.0.clone());
    let (nav, spots) = {
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).expect("the spatial query");
        let bounds = vis.as_deref().map(|vis| nav_gen::Bounds {
            vis,
            walker: chimera_sim::minigame::spawns::walker(),
        });
        let nav = nav_gen::generate_within(&q, &|_| false, &feet, &p, bounds);
        // The spawn spots' walker (`spawns::walker`).
        let spot_bounds = spot_bound.zip(vis.as_deref()).map(|(walker, vis)| nav_gen::Bounds { vis, walker });
        let spots = nav_gen::generate_within(&q, &|_| false, &feet, &spot_walker, spot_bounds);
        (nav, spots)
    };
    row.nav_s = t.elapsed().as_secs_f32();
    let size = |i: usize| {
        let s = nav.areas[i].max - nav.areas[i].min;
        s.x * s.y
    };
    row.mesh_m2 = (0..nav.areas.len()).map(size).sum();
    row.capped = row.mesh_m2 >= 0.95 * p.max_cells as f32 * p.cell * p.cell;
    {
        let (inside, _) = region(&spots, &feet);
        row.spot_area_m2 = (0..spots.areas.len())
            .filter(|i| inside[*i])
            .map(|i| {
                let s = spots.areas[i].max - spots.areas[i].min;
                s.x * s.y
            })
            .sum();
    }
    let (inside, starts) = region(&nav, &feet);
    let ids: Vec<usize> = (0..nav.areas.len()).filter(|i| inside[*i]).collect();
    row.areas = ids.len();
    row.area_m2 = ids.iter().map(|i| size(*i)).sum();
    // On the main region: over one of its areas (0.75 m slack) within
    // 1.5 m of its floor.
    let on_region = |q: Vec3| {
        ids.iter().any(|i| {
            let a = &nav.areas[*i];
            let (lo, hi) = (a.min - Vec2::splat(0.75), a.max + Vec2::splat(0.75));
            q.x >= lo.x && q.x <= hi.x && q.z >= lo.y && q.z <= hi.y && (a.height_at(q.x, q.z) - q.y).abs() < 1.5
        })
    };
    row.own_nav = own_nav;
    row.spots = candidates.len();
    let used: &[Vec3] = if candidates.is_empty() { &feet } else { &candidates };
    let off_box = |q: Vec3| playable.is_some_and(|(lo, hi)| q.cmplt(lo - 1.0).any() || q.cmpgt(hi + 1.0).any());
    let off_file = |q: Vec3| file_nav.as_ref().is_some_and(|n| party_kit::spot_checks::off_file(n, q));
    // The spots' main region: the navigation file's, else the bots' mesh
    // as the guard makes it (`spot_checks::bots_mesh`: fitting doorways as
    // the spots' walker does).
    let guard_mesh = if file_nav.is_none() { party_kit::spot_checks::bots_mesh(w, &feet) } else { NavMesh::default() };
    let spot_mesh = party_kit::spot_checks::region_mesh(file_nav.as_deref(), &guard_mesh);
    let (spot_inside, _) = region(spot_mesh, &feet);
    let on_spot_region = |q: Vec3| party_kit::spot_checks::on_region(spot_mesh, &spot_inside, q);
    // Where each lands (CS:S's float over their floors).
    let landed = party_kit::spot_checks::dropped(w, &feet);
    row.spawns_in_region = landed.iter().filter(|f| on_spot_region(**f)).count();
    if o.leaks && file_nav.is_some() {
        // Where the spots' walk leaves the navigation file: a link from an
        // area whose centre is on it to one whose centre is off it.
        let centre = |a: &chimera_world::map::nav::NavArea| {
            let c = (a.min + a.max) / 2.0;
            Vec3::new(c.x, a.height_at(c.x, c.y), c.y)
        };
        let (inside, _) = region(&spots, &feet);
        let mut seen = BTreeSet::new();
        for (i, a) in spots.areas.iter().enumerate() {
            if !inside[i] || off_file(centre(a)) {
                continue;
            }
            for (j, _) in &a.links {
                let b = &spots.areas[*j];
                if off_file(centre(b)) && seen.insert(((centre(b) / 2.0).round().as_ivec3()).to_array()) {
                    eprintln!("    leak: {} -> {}", at(centre(a)), at(centre(b)));
                }
            }
        }
    }
    if o.leaks
        && let Some(n) = &file_nav
    {
        // Where the navigation file goes on and the spots' walk doesn't:
        // a file link from an area the walk covers to one it doesn't (the
        // middle of the shared edge, as best the areas' boxes say).
        let (inside, _) = region(&spots, &feet);
        let walked: Vec<&chimera_world::map::nav::NavArea> =
            spots.areas.iter().enumerate().filter(|(i, _)| inside[*i]).map(|(_, a)| a).collect();
        let covered: Vec<bool> = n
            .areas
            .iter()
            .map(|a| {
                let c = (a.min + a.max) / 2.0;
                let y = a.height_at(c.x, c.y);
                walked.iter().any(|b| {
                    b.max.x > a.min.x && b.min.x < a.max.x && b.max.y > a.min.y && b.min.y < a.max.y && (b.center.y - y).abs() < 1.5
                })
            })
            .collect();
        let mut seen = BTreeSet::new();
        for (i, a) in n.areas.iter().enumerate() {
            if !covered[i] {
                continue;
            }
            for (j, via) in &a.links {
                if covered[*j] {
                    continue;
                }
                let b = &n.areas[*j];
                let lo = a.min.max(b.min);
                let hi = a.max.min(b.max);
                let m = (lo + hi) / 2.0;
                let p = Vec3::new(m.x, a.height_at(m.x, m.y).max(b.height_at(m.x, m.y)), m.y);
                if seen.insert((p / 2.0).round().as_ivec3().to_array()) {
                    eprintln!("    gap: {} ({via:?}, file areas {} -> {})", at(p), a.id, b.id);
                }
            }
        }
    }
    if o.leaks {
        // Where the bots' walk goes on and the spots' doesn't (a confined
        // map): a bots' link from an area the spots' main region covers
        // to one it doesn't.
        let (inside, _) = region(&spots, &feet);
        let walked: Vec<&chimera_world::map::nav::NavArea> =
            spots.areas.iter().enumerate().filter(|(i, _)| inside[*i]).map(|(_, a)| a).collect();
        let covered: Vec<bool> = nav
            .areas
            .iter()
            .map(|a| {
                walked.iter().any(|b| {
                    b.max.x > a.min.x && b.min.x < a.max.x && b.max.y > a.min.y && b.min.y < a.max.y && (b.center.y - a.center.y).abs() < 1.0
                })
            })
            .collect();
        let mut seen = BTreeSet::new();
        for i in &ids {
            if !covered[*i] {
                continue;
            }
            let a = &nav.areas[*i];
            for (j, _) in &a.links {
                if covered[*j] || !inside[*j] {
                    continue;
                }
                let b = &nav.areas[*j];
                if seen.insert((b.center / 2.0).round().as_ivec3().to_array()) {
                    eprintln!("    confined: {} -> {}", at(a.center), at(b.center));
                }
            }
        }
    }
    row.has_nav_file = file_nav.is_some();
    row.stuck = stuck_count;
    row.stuck_at = stuck_at;
    row.off_region = used.iter().filter(|q| !on_spot_region(**q)).count();
    row.off_nav_file = used.iter().filter(|q| off_file(**q)).count();
    row.off_box = used.iter().filter(|q| off_box(**q)).count();
    let off: Vec<Vec3> = used
        .iter()
        .copied()
        .filter(|q| !on_spot_region(*q) || off_file(*q) || off_box(*q))
        .collect();
    if o.dump {
        let file = o.out.join(format!("{}_spots.csv", id.replace(':', "_")));
        let mut text = String::from("x,y,z,off_region,off_nav_file,stuck\n");
        for q in used {
            let stuck = stuck_spots.contains(&q.to_array().map(f32::to_bits));
            let p = at(*q);
            let p = p.trim_end_matches(" m").replace(' ', ",");
            text.push_str(&format!("{p},{},{},{}\n", !on_spot_region(*q), off_file(*q), stuck));
        }
        if let Err(e) = fs::write(&file, text) {
            eprintln!("{}: {e}", file.display());
        }
        let file = o.out.join(format!("{}_points.csv", id.replace(':', "_")));
        let mut text = String::from("x,y,z,off_region\n");
        for q in &landed {
            let p = at(*q);
            text.push_str(&format!("{},{}\n", p.trim_end_matches(" m").replace(' ', ","), !on_spot_region(*q)));
        }
        let _ = fs::write(&file, text);
        {
            // The bots' walker's main region, a point every half metre.
            let file = o.out.join(format!("{}_mesh.csv", id.replace(':', "_")));
            let mut text = String::from("x,y,z\n");
            for i in &ids {
                let a = &nav.areas[*i];
                let size = a.max - a.min;
                let (nx, nz) = (((size.x * 2.0) as i32).max(1), ((size.y * 2.0) as i32).max(1));
                for k in 0..nx {
                    for l in 0..nz {
                        let x = a.min.x + (k as f32 + 0.5) * size.x / nx as f32;
                        let z = a.min.y + (l as f32 + 0.5) * size.y / nz as f32;
                        let p = at(Vec3::new(x, a.height_at(x, z), z));
                        text.push_str(&format!("{}\n", p.trim_end_matches(" m").replace(' ', ",")));
                    }
                }
            }
            let _ = fs::write(&file, text);
        }
        {
            // A top-down height map of the bots' region's bounds (the first
            // floor down from 2.5 m over its highest area), every 0.25 m.
            let (mut lo, mut hi, mut top) = (Vec2::MAX, Vec2::MIN, f32::MIN);
            for i in &ids {
                let a = &nav.areas[*i];
                lo = lo.min(a.min);
                hi = hi.max(a.max);
                top = top.max(a.center.y);
            }
            let file = o.out.join(format!("{}_floor.csv", id.replace(':', "_")));
            let mut text = String::from("x,y,z\n");
            let mut state = SystemState::<SpatialQuery>::new(w);
            let q = state.get(w).expect("the spatial query");
            let filter = SpatialQueryFilter::from_mask(chimera_core::core::CHARACTER_FILTER);
            let mut x = lo.x;
            while x <= hi.x && ids.len() > 0 {
                let mut z = lo.y;
                while z <= hi.y {
                    let from = Vec3::new(x, top + 2.5, z);
                    if let Some(h) = q.cast_ray(from, Dir3::NEG_Y, 40.0, true, &filter) {
                        let p = at(from - Vec3::Y * h.distance);
                        text.push_str(&format!("{}\n", p.trim_end_matches(" m").replace(' ', ",")));
                    }
                    z += 0.25;
                }
                x += 0.25;
            }
            let _ = fs::write(&file, text);
        }
        if let Some(ents) = w.get_resource::<chimera_world::map::MapEntities>() {
            let file = o.out.join(format!("{}_entities.txt", id.replace(':', "_")));
            let mut text = String::new();
            for (i, e) in ents.entities.iter().enumerate() {
                let kv: Vec<String> = e.keyvalues.iter().map(|(k, v)| format!("{k}={v}")).collect();
                text.push_str(&format!("{i} hulls={} mover={} {}\n", e.hulls.len(), e.mover, kv.join(" | ")));
            }
            let _ = fs::write(&file, text);
        }
        if let Some(n) = &file_nav {
            let file = o.out.join(format!("{}_nav.csv", id.replace(':', "_")));
            let mut text = String::from("x,y,z\n");
            for a in &n.areas {
                // The area's points a metre apart (its centre at least).
                let size = a.max - a.min;
                let (nx, nz) = ((size.x as i32).max(1), (size.y as i32).max(1));
                for i in 0..nx {
                    for k in 0..nz {
                        let x = a.min.x + (i as f32 + 0.5) * size.x / nx as f32;
                        let z = a.min.y + (k as f32 + 0.5) * size.y / nz as f32;
                        let p = at(Vec3::new(x, a.height_at(x, z), z));
                        text.push_str(&format!("{}\n", p.trim_end_matches(" m").replace(' ', ",")));
                    }
                }
            }
            let _ = fs::write(&file, text);
        }
    }
    row.spots_off = off.len();
    row.spots_off_at = off
        .iter()
        .step_by((off.len() / 6).max(1))
        .take(6)
        .map(|q| at(*q))
        .collect::<Vec<_>>()
        .join("; ");
    if ids.is_empty() {
        row.error = Some("no walkable region from the spawn points".into());
        return row;
    }
    let v = o.speed;

    // Area-weighted samples (seeded): uniform points of the region.
    let weights: Vec<f32> = ids.iter().map(|i| size(*i)).collect();
    let mut rng = SplitMix(0x6d61_705f_6669_746e);
    let mut cumulative = Vec::with_capacity(weights.len());
    let mut acc = 0.0f32;
    for w in &weights {
        acc += w;
        cumulative.push(acc);
    }
    let draw = |rng: &mut SplitMix| {
        let x = rng.unit() * acc;
        ids[cumulative.partition_point(|c| *c < x).min(ids.len() - 1)]
    };
    let samples: Vec<usize> = (0..o.samples).map(|_| draw(&mut rng)).collect();

    // Sight: from each sample's eye (1.6 m over its area's floor), 16
    // level rays against the world, each capped at `SIGHT_CAP`.
    {
        let mut state = SystemState::<SpatialQuery>::new(w);
        let q = state.get(w).expect("the spatial query");
        let filter = SpatialQueryFilter::from_mask(p.mask);
        let mut sum = 0.0;
        let mut count = 0;
        for s in &samples {
            let a = &nav.areas[*s];
            let c = (a.min + a.max) / 2.0;
            let eye = Vec3::new(c.x, a.height_at(c.x, c.y) + 1.6, c.y);
            for k in 0..16 {
                let t = k as f32 * std::f32::consts::TAU / 16.0;
                let d = Dir3::new(Vec3::new(t.cos(), 0.0, t.sin())).unwrap();
                sum += q
                    .cast_ray(eye, d, SIGHT_CAP, true, &filter)
                    .map_or(SIGHT_CAP, |h| h.distance);
                count += 1;
            }
        }
        row.sight_m = sum / count.max(1) as f32;
    }
    let from_samples: Vec<Vec<f32>> = samples.iter().map(|s| dijkstra(&nav, *s)).collect();

    // Random pairs: each sample to the whole region, weighted by area.
    let mut pairs: Vec<(f32, f32)> = Vec::new();
    for d in &from_samples {
        for (k, i) in ids.iter().enumerate() {
            if d[*i].is_finite() {
                pairs.push((d[*i], weights[k]));
            }
        }
    }
    row.pair = (wpct(&mut pairs, 0.5) / v, wpct(&mut pairs, 0.9) / v);

    // The hub: least mean distance from the samples.
    let hub = ids
        .iter()
        .copied()
        .min_by(|a, b| {
            let mean = |i: usize| from_samples.iter().map(|d| d[i].min(1e6)).sum::<f32>();
            mean(*a).total_cmp(&mean(*b))
        })
        .unwrap();
    let to_hub = dijkstra(&nav, hub);

    // Spawn points to each other and to the hub.
    let spawn_areas: Vec<usize> = starts.iter().flatten().copied().filter(|s| inside[*s]).collect();
    let mut sp = Vec::new();
    let mut sh = Vec::new();
    for (k, a) in spawn_areas.iter().enumerate() {
        let d = dijkstra(&nav, *a);
        for b in &spawn_areas[k + 1..] {
            if d[*b].is_finite() {
                sp.push(d[*b] / v);
            }
        }
        if to_hub[*a].is_finite() {
            sh.push(to_hub[*a] / v);
        }
    }
    sp.sort_by(f32::total_cmp);
    sh.sort_by(f32::total_cmp);
    row.spawn_pair = (pct(&sp, 0.5), pct(&sp, 0.9));
    row.spawn_hub = (pct(&sh, 0.5), pct(&sh, 0.9));

    // N players spread at random: each one's walk to the nearest other.
    for (slot, n) in N_PLAYERS.iter().enumerate() {
        let mut nearest = Vec::new();
        for _ in 0..400 {
            let picks: Vec<usize> = (0..*n).map(|_| rng.below(samples.len())).collect();
            for (a, pa) in picks.iter().enumerate() {
                let best = picks
                    .iter()
                    .enumerate()
                    .filter(|(b, _)| *b != a)
                    .map(|(_, pb)| from_samples[*pa][samples[*pb]])
                    .fold(f32::INFINITY, f32::min);
                if best.is_finite() {
                    nearest.push(best / v);
                }
            }
        }
        nearest.sort_by(f32::total_cmp);
        row.meet[slot] = pct(&nearest, 0.5);
    }
    // Wandering: N players walking about at random meet at the rate of a
    // gas's collisions, cross-section twice the sight, relative speed
    // 4/pi of the run speed (random directions): the mean time to a
    // player's next meeting, area / ((N - 1) * 2 * sight * 4/pi * speed).
    for (slot, n) in WANDER_PLAYERS.iter().enumerate() {
        row.wander[slot] = row.area_m2 / ((*n as f32 - 1.0) * 2.0 * row.sight_m * 4.0 / std::f32::consts::PI * v);
    }
    if !o.probes.is_empty() {
        sim.seconds(5.0);
        probe_report(&mut sim, &o.probes, &at);
    }
    drop(sim);
    row
}

/// The areas walked to from a spawn point and back (the spawn spots'
/// region, `chimera_sim::minigame::spawns`), and each spawn point's area.
fn region(nav: &NavMesh, feet: &[Vec3]) -> (Vec<bool>, Vec<Option<usize>>) {
    party_kit::spot_checks::region(nav, feet)
}

#[derive(PartialEq)]
struct Item(f32, usize);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// Walking distance (m, centre to centre) from area `from` to every area.
fn dijkstra(nav: &NavMesh, from: usize) -> Vec<f32> {
    let mut d = vec![f32::INFINITY; nav.areas.len()];
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
                heap.push(Item(next, *j));
            }
        }
    }
    d
}

fn pct(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    sorted[((sorted.len() - 1) as f32 * q).round() as usize]
}

/// The weighted `q` quantile of (value, weight).
fn wpct(items: &mut [(f32, f32)], q: f32) -> f32 {
    if items.is_empty() {
        return f32::NAN;
    }
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f32 = items.iter().map(|i| i.1).sum();
    let mut acc = 0.0;
    for (v, w) in items.iter() {
        acc += w;
        if acc >= q * total {
            return *v;
        }
    }
    items.last().unwrap().0
}

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn used_spots<'a>(candidates: &'a [Vec3], feet: &'a [Vec3]) -> &'a [Vec3] {
    if candidates.is_empty() { feet } else { candidates }
}

/// After the map ran a few seconds: what a standing box at each probe
/// meets (with 5 cm to spare around it too), the floor under it and the
/// ceiling over it.
fn probe_report(sim: &mut Sim, probes: &[Vec3], at: &dyn Fn(Vec3) -> String) {
    let w = sim.app.world_mut();
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).expect("the spatial query");
    let filter = SpatialQueryFilter::from_mask(chimera_core::core::CHARACTER_FILTER);
    let mut lines = Vec::new();
    for p in probes {
        let centre = *p + Vec3::Y * BOX_CENTRE;
        let mut tight = Vec::new();
        q.shape_intersections_callback(&player_box(), centre, Quat::IDENTITY, &filter, |e| {
            tight.push(e);
            true
        });
        let mut loose = Vec::new();
        let wide = Collider::cuboid(PLAYER_W + 0.1, PLAYER_H, PLAYER_W + 0.1);
        q.shape_intersections_callback(&wide, *p + Vec3::Y * (PLAYER_H / 2.0), Quat::IDENTITY, &filter, |e| {
            loose.push(e);
            true
        });
        let floor = q
            .cast_ray(*p + Vec3::Y * 0.5, Dir3::NEG_Y, 3.0, true, &filter)
            .map(|h| (h.distance - 0.5, h.entity));
        let ceiling = q
            .cast_ray(*p + Vec3::Y * 0.1, Dir3::Y, 5.0, true, &filter)
            .map(|h| (h.distance + 0.1, h.entity));
        // Anything at all under it (every layer), nearest first.
        let mut any = Vec::new();
        let every = SpatialQueryFilter::from_mask(LayerMask::ALL);
        q.ray_hits_callback(*p + Vec3::Y * 0.5, Dir3::NEG_Y, 3.0, true, &every, |h| {
            any.push((h.distance - 0.5, h.entity));
            true
        });
        any.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut around = Vec::new();
        q.shape_intersections_callback(&Collider::cuboid(8.0, 6.0, 8.0), *p, Quat::IDENTITY, &every, |e| {
            around.push(e);
            true
        });
        eprintln!(
            "    within 4 m, every layer: {:?}",
            around
                .iter()
                .map(|e| {
                    let n = |e: Entity| w.get::<Name>(e).map(|n| n.to_string()).unwrap_or_else(|| e.to_string());
                    format!(
                        "{} ({})",
                        n(*e),
                        w.get::<ChildOf>(*e).map(|c| n(c.parent())).unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
        );
        eprintln!(
            "    under it, every layer: {:?}",
            any.iter().map(|(d, e)| format!("{d:.2} {e}")).collect::<Vec<_>>()
        );
        if let Some(b) = w.get_resource::<chimera_world::map::MapBrushes>() {
            let skip: BTreeSet<usize> = w
                .get_resource::<chimera_core::core::MapPlayerClips>()
                .map(|s| s.0.iter().copied().collect())
                .unwrap_or_default();
            let half = Vec3::new(PLAYER_W, PLAYER_H - STEP - 0.05, PLAYER_W) / 2.0;
            let hit: Vec<String> = b
                .0
                .iter()
                .enumerate()
                .filter(|(_, x)| x.overlaps_box(centre, half, 0.0))
                .map(|(i, x)| format!("{i}{} {}..{}", if skip.contains(&i) { " (clip)" } else { "" }, at(x.min), at(x.max)))
                .collect();
            eprintln!("    brushes the box meets (of {}): {hit:?}", b.0.len());
        }
        lines.push((*p, tight, loose, floor, ceiling));
    }
    let name = |e: Entity| {
        let n = |e: Entity| w.get::<Name>(e).map(|n| n.to_string());
        format!(
            "{} ({})",
            n(e).unwrap_or_else(|| e.to_string()),
            w.get::<ChildOf>(e).and_then(|c| n(c.parent())).unwrap_or_default()
        )
    };
    for (p, tight, loose, floor, ceiling) in lines {
        let list = |v: &[Entity]| {
            if v.is_empty() {
                "nothing".to_string()
            } else {
                v.iter().map(|e| name(*e)).collect::<Vec<_>>().join("; ")
            }
        };
        eprintln!(
            "  probe {} after 5 s: box meets {}; with 5 cm more {}; floor {}; ceiling {}",
            at(p),
            list(&tight),
            list(&loose),
            floor.map_or("none within 2.5 m".into(), |(d, e)| format!(
                "{:.2} m down, {}",
                d,
                name(e)
            )),
            ceiling.map_or("none within 5 m".into(), |(d, e)| format!("{:.2} m up, {}", d, name(e))),
        );
    }
}
