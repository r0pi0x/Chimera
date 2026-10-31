//! Every ladder on the stock maps (and, with `CHIMERA_LADDER_MAPS`, any
//! others: a comma-separated list of map names, or `@file` holding one),
//! headless: climbing plays CS:S's ladder steps (specs/cs_source/sounds.md,
//! "Ladder", or a footstep volume's replacement for them, as on
//! cs_militia; the nav mesh's ladders walked onto from their foot, or each
//! ladder brush on a map without them), and standing on a ladder brush's
//! top is standing on a floor (no ladder mode: walking forward works and
//! a jump is a normal jump). Skipped without an install.
//! `CHIMERA_LADDER_DEBUG=1` prints each try; `CHIMERA_LADDER_TRACE=<nav
//! ladder index>` that climb tick by tick.

use avian3d::prelude::Position;
use bevy::prelude::*;
use crate::stack::{
    core::{SimSet, Velocity},
    games::{
        self, cs_source,
        cs_source::movement::{self, SourceMovement, SourceMovementPlugin},
    },
    harness::{CsSim, Sim},
    map::{MapBrush, MapData, MapPlugin, PlaySound, entities::entity_to_engine},
    mount::config::{LocalConfig, content_dir},
};

const M: f32 = 0.0254;

/// Ticks on a ladder that must hear a ladder step: more than a step period
/// (450 ms). The step timer is shared with the ground's, so a climb shorter
/// than what is left of the last ground step's 400 ms (a train car's
/// ladder taken looking up) is silent by the spec's rules.
const LONG_CLIMB: u32 = 30;

const STOCK: [&str; 18] = [
    "cs_assault",
    "cs_compound",
    "cs_havana",
    "cs_italy",
    "cs_militia",
    "cs_office",
    "de_aztec",
    "de_cbble",
    "de_chateau",
    "de_dust",
    "de_dust2",
    "de_inferno",
    "de_nuke",
    "de_piranesi",
    "de_port",
    "de_prodigy",
    "de_tides",
    "de_train",
];

fn load(name: &str) -> Option<MapData> {
    LocalConfig::load().ok()?.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())?;
    // The install's maps, or the asset store's standing in for it.
    let stock = ::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{name}.bsp")));
    let cached = content_dir(cs_source::GAME).is_some_and(|d| d.join("maps").join(format!("{name}.bsp")).is_file());
    if !stock && !cached {
        eprintln!("skipping {name}: not installed");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// The maps `CHIMERA_LADDER_MAPS` lists, if set: they replace the stock
/// maps.
fn listed() -> Option<Vec<String>> {
    let list = chimera_core::env::var("CHIMERA_LADDER_MAPS").ok()?;
    // `@file`: the list is in that file.
    Some(
        match list.strip_prefix('@') {
            Some(file) => std::fs::read_to_string(file).expect("the map list file"),
            None => list,
        }
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect(),
    )
}

/// One stock map's ladders, unless `CHIMERA_LADDER_MAPS` names others.
fn stock(name: &str) {
    if listed().is_none() {
        every_ladder_steps_and_its_top_is_a_floor(vec![name.to_string()]);
    }
}

#[derive(Resource, Default)]
struct Heard(Vec<PlaySound>);

fn listen(mut m: MessageReader<PlaySound>, mut heard: ResMut<Heard>) {
    heard.0.extend(m.read().cloned());
}

struct Tester {
    sim: Sim,
    p: Entity,
    /// The entries that count as ladder steps: Ladder.StepLeft/Right, and
    /// the steps of the surfaces footstep volumes play for "ladder"
    /// (cs_militia's func_footstep_control).
    steps: Vec<String>,
}

impl Tester {
    fn new(map: &MapData) -> Self {
        let mut sim = Sim::new(CsSim, (MapPlugin::new(map.clone()), SourceMovementPlugin));
        sim.set_tick_interval(cs_source::TICK_INTERVAL);
        sim.app
            .init_resource::<Heard>()
            .add_systems(FixedUpdate, listen.after(SimSet::Movement));
        let p = sim.spawn_character(Vec3::ZERO, movement::ID);
        let mut steps = vec!["Ladder.StepLeft".to_string(), "Ladder.StepRight".to_string()];
        for r in map.sounds.step_remaps.iter().filter(|r| r.from == "ladder") {
            let s = map.sounds.surface(&r.to);
            steps.extend(s.into_iter().flat_map(|s| [s.step_left.clone(), s.step_right.clone()]).flatten());
        }
        Self { sim, p, steps }
    }

    /// Ladder steps (`steps`) heard since the last call.
    fn ladder_steps(&mut self) -> usize {
        let heard = self.heard();
        heard.iter().filter(|s| self.steps.iter().any(|e| e.eq_ignore_ascii_case(&s.entry))).count()
    }

    /// Put the character's transform at `at` (engine), at rest, facing
    /// engine direction `facing` (horizontal), pitch in degrees (positive
    /// looks down), no keys.
    fn place(&mut self, at: Vec3, facing: Vec3, pitch: f32) {
        let w = self.sim.app.world_mut();
        let mut e = w.entity_mut(self.p);
        e.get_mut::<Transform>().unwrap().translation = at;
        if let Some(mut pos) = e.get_mut::<Position>() {
            pos.0 = at;
        }
        e.get_mut::<Velocity>().unwrap().0 = Vec3::ZERO;
        *e.get_mut::<SourceMovement>().unwrap() = SourceMovement::default();
        let mut i = self.sim.intent(self.p);
        *i = default();
        // Intent yaw 0 looks down engine -Z.
        i.yaw = (-facing.x).atan2(-facing.z);
        i.pitch = -pitch.to_radians();
    }

    fn state(&self) -> SourceMovement {
        self.sim.app.world().get::<SourceMovement>(self.p).unwrap().clone()
    }

    fn heard(&mut self) -> Vec<PlaySound> {
        std::mem::take(&mut self.sim.app.world_mut().resource_mut::<Heard>().0)
    }
}

/// Climb `b` from in front of one of its four sides: whether it climbed
/// (on the ladder 8 ticks, rising 20 units): the ladder steps heard and the ticks on it.
fn climb(t: &mut Tester, b: &MapBrush) -> Option<(usize, u32)> {
    let centre = (b.min + b.max) / 2.0;
    let height = (b.max.y - b.min.y) / M;
    // Feet 64 units above the ladder's bottom, settled on whatever is in
    // front (the floor may be raised); in the air at a third of its height
    // (train cars' ladders start above the floor), grabbing it at once;
    // from the floor 48 units out, walking in. The transform is 36 up.
    let starts = [
        (64.0f32.min(height / 2.0), true, 0.0),
        (height / 3.0, false, 0.0),
        (0.0, true, 48.0),
    ];
    for (up, settle, out) in starts {
        for d in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            let reach = (b.max - b.min).dot(d.abs()) / 2.0 + (17.0 + out) * M;
            let at = Vec3::new(centre.x, b.min.y + (up + 36.0) * M, centre.z) + d * reach;
            // Looking a little up, as players climb.
            t.place(at, -d, -20.0);
            if settle {
                t.sim.ticks(32);
            }
            t.heard();
            t.sim.intent(t.p).move_axis = Vec2::Y;
            let y0 = t.sim.position(t.p).y;
            let (mut on, mut rise) = (0, 0.0f32);
            for _ in 0..96 {
                t.sim.ticks(1);
                on +=t.state().ladder.is_some() as u32;
                rise = rise.max((t.sim.position(t.p).y - y0) / M);
            }
            if chimera_core::env::var_os("CHIMERA_LADDER_DEBUG").is_some() {
                eprintln!("  try {d} up {up} out {out}: on {on} ticks, rose {rise:.0}, at {:?}", t.sim.position(t.p) / M);
            }
            // On it a while, climbing 20 units at least.
            if on >= 8 && rise > 20.0 {
                let steps = t.ladder_steps();
                return Some((steps, on));
            }
        }
    }
    None
}

/// Walk onto nav ladder `l` from 24 units out at its foot, facing it and
/// looking a little up, holding forward: the ladder steps heard, when it
/// climbed (on it 8 ticks and rose 20 units), and the ticks on it.
fn climb_nav(t: &mut Tester, l: &crate::stack::map::nav::NavLadder, trace: bool) -> Option<(usize, u32)> {
    let out = l.normal.with_y(0.0).normalize_or_zero();
    let at = l.bottom + out * 24.0 * M + Vec3::Y * 40.0 * M;
    t.place(at, -out, -20.0);
    t.sim.ticks(32);
    t.heard();
    t.sim.intent(t.p).move_axis = Vec2::Y;
    let y0 = t.sim.position(t.p).y;
    let (mut on, mut rise) = (0, 0.0f32);
    for _ in 0..160 {
        t.sim.ticks(1);
        on += t.state().ladder.is_some() as u32;
        rise = rise.max((t.sim.position(t.p).y - y0) / M);
        if trace {
            let s = t.state();
            eprintln!(
                "    ladder {:?} ground {} ducked {} v {:?} y {:.1} timer {}",
                s.ladder,
                s.on_ground,
                s.ducked,
                t.sim.velocity(t.p) / M,
                t.sim.position(t.p).y / M,
                s.step_timer
            );
        }
    }
    if chimera_core::env::var_os("CHIMERA_LADDER_DEBUG").is_some() {
        eprintln!("  nav try from {:?} out {out}: on {on} ticks, rose {rise:.0}, at {:?}", at / M, t.sim.position(t.p) / M);
    }
    (on >= 8 && rise > 20.0).then(|| (t.ladder_steps(), on))
}

/// Stand on `b`'s top (when there's room): holding forward one tick in
/// each direction, looking 30 degrees down, must not grab the top as a
/// ladder; a jump from standing must rise at the jump speed. None: no
/// room to stand there. Some(problems).
fn top(t: &mut Tester, b: &MapBrush) -> Option<Vec<String>> {
    let centre = (b.min + b.max) / 2.0;
    let at = Vec3::new(centre.x, b.max.y + 36.5 * M, centre.z);
    t.place(at, Vec3::NEG_Z, 0.0);
    t.sim.ticks(8);
    let feet = t.sim.position(t.p).y - 36.0 * M;
    let s = t.state();
    if !s.on_ground || (feet - b.max.y).abs() > 1.0 * M || (t.sim.position(t.p) - at).xz().length() > 1.0 * M {
        return None;
    }
    let mut problems = Vec::new();
    for d in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
        t.place(at, d, 30.0);
        t.sim.ticks(4);
        t.sim.intent(t.p).move_axis = Vec2::Y;
        t.sim.ticks(1);
        if let Some(n) = t.state().ladder.filter(|n| n.z.abs() > 0.7) {
            problems.push(format!("walking {d}: on the top as a ladder (normal {n})"));
        }
    }
    t.place(at, Vec3::NEG_Z, 30.0);
    t.sim.ticks(4);
    t.sim.intent(t.p).jump = true;
    t.sim.ticks(1);
    let vz = t.sim.velocity(t.p).y / M;
    if vz > 320.0 {
        problems.push(format!("jump: {vz:.0} units/s up"));
    }
    Some(problems)
}

/// Every ladder of `maps` steps and its top is a floor (one test per stock
/// map below, so they run side by side; `listed_maps` for
/// `CHIMERA_LADDER_MAPS`).
fn every_ladder_steps_and_its_top_is_a_floor(maps: Vec<String>) {
    let mut report = Vec::new();
    let (mut silent_total, mut stuck_total) = (0, 0);
    for name in maps {
        let Some(map) = load(&name) else { continue };
        let ladders: Vec<MapBrush> = map.collision_brushes.iter().filter(|b| b.ladder).cloned().collect();
        if !ladders.is_empty() {
            // The steps' waves load and decode on every map with ladders.
            for side in ["Ladder.StepLeft", "Ladder.StepRight"] {
                let e = map.sounds.entry(side).unwrap_or_else(|| panic!("{name}: no {side}"));
                assert!(!e.waves.is_empty(), "{name}: {side} has no waves");
                for w in &e.waves {
                    assert!(!map.sounds.clips[*w].samples.is_empty(), "{name}: {side} wave {w} empty");
                }
            }
        }
        let mut t = Tester::new(&map);
        let (mut climbed, mut silent, mut tops, mut stuck) = (0, Vec::new(), 0, Vec::new());
        // Climbs too short to be due a step (no step heard).
        let mut short = 0;
        // Climbing each brush is slow (most tries are blocked): only on maps
        // whose nav mesh has no ladders.
        let brush_climbs = chimera_core::env::var_os("CHIMERA_LADDER_NAV_ONLY").is_none()
            && map.kit.nav.as_ref().is_none_or(|n| n.ladders.is_empty());
        for (i, b) in ladders.iter().enumerate().filter(|_| brush_climbs) {
            match climb(&mut t, b) {
                Some((0, on)) if on >= LONG_CLIMB => silent.push(i),
                Some((0, _)) => short += 1,
                Some(_) => {}
                None => {
                    if chimera_core::env::var_os("CHIMERA_LADDER_DEBUG").is_some() {
                        eprintln!("{name} #{i}: not climbed, {:?}..{:?} units", b.min / M, b.max / M);
                    }
                    continue;
                }
            }
            climbed += 1;
        }
        // The nav mesh's ladders (where the map has one): walked onto from
        // their foot as bots and players do.
        let (mut nav_climbed, mut nav_silent, mut nav_total) = (0, Vec::new(), 0);
        if let Some(nav) = map.kit.nav.clone() {
            nav_total = nav.ladders.len();
            for (i, l) in nav.ladders.iter().enumerate() {
                let trace = chimera_core::env::var("CHIMERA_LADDER_TRACE").is_ok_and(|v| v == i.to_string());
                match climb_nav(&mut t, l, trace) {
                    Some((0, on)) if on >= LONG_CLIMB => nav_silent.push(i),
                    Some((0, _)) => short += 1,
                    Some(_) => {}
                    None => {
                        if chimera_core::env::var_os("CHIMERA_LADDER_DEBUG").is_some() {
                            eprintln!("{name} nav ladder {i}: not climbed, {:?} to {:?}", l.bottom / M, l.top / M);
                        }
                        continue;
                    }
                }
                nav_climbed += 1;
            }
        }
        for (i, b) in ladders.iter().enumerate() {
            if let Some(p) = top(&mut t, b) {
                tops += 1;
                if !p.is_empty() {
                    stuck.push(format!("#{i} at {:?}: {}", (b.min + b.max) / 2.0 / M, p.join("; ")));
                }
            }
        }
        report.push(format!(
            "{name}: {} ladder brushes, {climbed} climbed, silent {silent:?}; nav ladders {nav_climbed} of {nav_total} climbed, silent {nav_silent:?}; {short} short climbs heard no step; {tops} tops stood on, {} wrong: {stuck:?}",
            ladders.len(),
            stuck.len(),
        ));
        silent_total += silent.len() + nav_silent.len();
        stuck_total += stuck.len();
    }
    eprintln!("{}", report.join("\n"));
    assert_eq!(silent_total, 0, "silent ladders:\n{}", report.join("\n"));
    assert_eq!(stuck_total, 0, "ladder tops that aren't floors:\n{}", report.join("\n"));
}

/// cs_militia's two func_footstep_control volumes ("source" ladder,
/// "destination" concrete and wood_plank) load as step remaps where their
/// brushes are, and climbing a ladder whose path runs through one plays
/// the destination's steps there (`games::cs_source::sound::step_remaps`).
#[test]
fn militia_footstep_volumes_remap_ladder_steps() {
    let Some(map) = load("cs_militia") else { return };
    let sounds = map.sounds.clone();
    let mut pairs: Vec<(&str, &str)> = sounds.step_remaps.iter().map(|r| (r.from.as_str(), r.to.as_str())).collect();
    pairs.sort();
    assert_eq!(pairs, [("ladder", "concrete"), ("ladder", "wood_plank")]);
    for e in map.entities.iter().filter(|e| e.classname() == "func_footstep_control") {
        let points: Vec<Vec3> =
            e.hulls.iter().flat_map(|h| h.points.iter()).map(|p| entity_to_engine(*p + e.origin(), M)).collect();
        assert!(!points.is_empty(), "a footstep volume without brushes");
        let centre = points.iter().sum::<Vec3>() / points.len() as f32;
        let to = e.get("destination").unwrap();
        assert_eq!(sounds.step_surface("ladder", centre), to, "inside at {:?}", centre / M);
        assert_eq!(sounds.step_surface("concrete", centre), "concrete", "other surfaces unchanged");
        assert_eq!(sounds.step_surface("ladder", centre + Vec3::Y * 2000.0 * M), "ladder", "outside");
    }
    // Nav ladders whose climb passes through a volume (one into
    // wood_plank, one into concrete): climbed, they play the
    // destination's steps.
    let nav = map.kit.nav.clone().expect("cs_militia's nav mesh");
    let mut t = Tester::new(&map);
    let mut climbed = Vec::new();
    for (i, l) in nav.ladders.iter().enumerate() {
        let Some(r) = (0..=20)
            .map(|k| l.bottom.lerp(l.top, k as f32 / 20.0) + l.normal * 16.0 * M)
            .find_map(|p| sounds.step_remaps.iter().find(|r| r.from == "ladder" && r.contains(p)))
        else {
            continue;
        };
        let s = sounds.surface(&r.to).expect("the destination surface");
        t.steps = [s.step_left.clone(), s.step_right.clone()].into_iter().flatten().collect();
        // Not every nav ladder climbs from its foot (`stock` counts them).
        let Some((steps, _)) = climb_nav(&mut t, l, false) else { continue };
        assert!(steps > 0, "nav ladder {i} (into the {} volume): no {:?} heard", r.to, t.steps);
        climbed.push(r.to.clone());
    }
    assert!(climbed.iter().any(|to| to == "wood_plank"), "the ladder into wood_plank climbed: {climbed:?}");
}

/// The maps `CHIMERA_LADDER_MAPS` names (nothing without it).
#[test]
fn listed_maps() {
    if let Some(maps) = listed() {
        every_ladder_steps_and_its_top_is_a_floor(maps);
    }
}

macro_rules! stock_maps {
    ($($map:ident,)*) => {
        /// One test per stock map (`STOCK`), so they spread over the heavy
        /// tests' slots.
        mod every_ladder_steps_and_its_top_is_a_floor_on {
            $(
                #[test]
                fn $map() {
                    super::stock(stringify!($map));
                }
            )*

            #[test]
            fn covers_every_stock_map() {
                assert_eq!([$(stringify!($map)),*], super::STOCK);
            }
        }
    };
}

stock_maps! {
    cs_assault, cs_compound, cs_havana, cs_italy, cs_militia, cs_office, de_aztec, de_cbble, de_chateau,
    de_dust, de_dust2, de_inferno, de_nuke, de_piranesi, de_port, de_prodigy, de_tides, de_train,
}
