//! Combat Arms' movement in the maps' volumes (specs/combat_arms/
//! movement.md "Other modifiers", world_dat.md "Moving world models and
//! volumes") on the user's install: a census of the volume classes the
//! player's movement reads over every map, and players in the maps that
//! have them. Passes with a note without `[games.combat_arms] path` in
//! chimera.local.toml (and the maps in keyed archives without the V2 key).

use bevy::prelude::*;
use chimera_core::{
    core::{Health, Velocity},
    mount::{Mount, config::LocalConfig},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        GAME, INFO,
        movement::{self, CaMovement, movement_data},
        volumes::CaVolume,
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{Value, World};

fn mount() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// The volume classes the movement reads.
const MOVEMENT_CLASSES: &[&str] = &[
    "Water",
    "CorrosiveFluid",
    "FreezingWater",
    "Ladder",
    "Gravity",
    "JumpVolume",
    "SafteyNet",
    "Ice",
    "Wind",
    "VolumeBrush",
];

fn value(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s:?}"),
        Value::Vector(v) | Value::Color(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
        Value::Real(r) | Value::Flags(r) | Value::Long(r) => format!("{r}"),
        Value::Bool(b) => format!("{b}"),
        other => format!("{other:?}"),
    }
}

/// Every readable map's movement volumes: class, name, the properties the
/// movement reads and the world model's bounds (file axes, u). Prints the
/// census; with all 105 maps readable (the V2 key), checks the counts.
#[test]
fn volume_census() {
    let Some(mount) = mount() else { return };
    let keyed = combat_arms::game::mount::can_read("V2");
    let mut per_class: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut read = 0;
    for name in world::map_names(&mount) {
        let Some(path) = world::find(&mount, &name) else { continue };
        let Ok(bytes) = mount.read(&path) else {
            eprintln!("{name}: not readable (keyed: {keyed})");
            continue;
        };
        let Ok(w) = World::parse(&bytes) else {
            eprintln!("{name}: does not parse");
            continue;
        };
        read += 1;
        for o in w.objects.iter().filter(|o| MOVEMENT_CLASSES.contains(&o.class.as_str())) {
            let bounds = o
                .name()
                .and_then(|n| w.model(n))
                .map(|(_, m)| {
                    let pts: Vec<Vec3> = (0..m.polygons.len())
                        .flat_map(|i| m.polygon_vertices(i).collect::<Vec<_>>())
                        .map(Vec3::from)
                        .collect();
                    let lo = pts.iter().fold(Vec3::MAX, |a, p| a.min(*p));
                    let hi = pts.iter().fold(Vec3::MIN, |a, p| a.max(*p));
                    format!("{lo:.0}..{hi:.0}")
                })
                .unwrap_or_else(|| "no model".into());
            let props: Vec<String> = [
                "Viscosity",
                "Current",
                "Friction",
                "PlayerSpeedFactor",
                "PlayerGravity",
                "Velocity",
                "Damage",
                "Hidden",
            ]
            .iter()
            .filter_map(|k| o.get(k).map(|v| format!("{k} {}", value(v))))
            .collect();
            per_class
                .entry(o.class.clone())
                .or_default()
                .push(format!("{name} {:?} {bounds} {}", o.name().unwrap_or("-"), props.join(", ")));
        }
    }
    eprintln!("{read} maps read");
    for (class, list) in &per_class {
        eprintln!("{class}: {} in {} maps", list.len(), {
            let mut maps: Vec<&str> = list.iter().map(|l| l.split(' ').next().unwrap()).collect();
            maps.dedup();
            maps.len()
        });
        for l in list {
            eprintln!("    {l}");
        }
    }
    // With the V2 key, all 105 maps: the counts this module's tests and
    // the plan quote.
    if read == 105 {
        let count = |c: &str| per_class.get(c).map_or(0, Vec::len);
        let maps = |c: &str| {
            let mut m: Vec<&str> =
                per_class.get(c).into_iter().flatten().map(|l| l.split(' ').next().unwrap()).collect();
            m.dedup();
            m.len()
        };
        let got: Vec<(&str, usize, usize)> = ["Water", "Ladder", "JumpVolume", "Gravity", "SafteyNet", "Ice", "Wind"]
            .into_iter()
            .map(|c| (c, count(c), maps(c)))
            .collect();
        assert_eq!(
            got,
            [
                ("Water", 20, 18),
                ("Ladder", 266, 55),
                ("JumpVolume", 42, 3),
                ("Gravity", 19, 2),
                ("SafteyNet", 21, 6),
                ("Ice", 0, 0),
                ("Wind", 0, 0)
            ]
        );
    }
}

/// A map from the install in a simulation, with a player at `at`
/// (engine metres; dropped to the floor below on its first tick).
fn sim_on(mount: &Mount, name: &str, at: Vec3) -> (Sim, Entity) {
    let map = world::load_world(mount, name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(at, movement::ID);
    sim.ticks(1);
    (sim, p)
}

fn mv(sim: &Sim, p: Entity) -> CaMovement {
    sim.app.world().get::<CaMovement>(p).unwrap().clone()
}

/// Put the player's centre at `at` (u, file axes) at rest in the air.
fn place(sim: &mut Sim, p: Entity, at: Vec3) {
    let s = movement_data().meters_per_unit;
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = file_to_engine(at) * s;
    w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    w.get_mut::<CaMovement>(p).unwrap().standing = None;
}

/// File axes (u) to engine axes (u): Z mirrored.
fn file_to_engine(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.y, -p.z)
}

fn file_y(sim: &Sim, p: Entity) -> f32 {
    sim.position(p).y / movement_data().meters_per_unit
}

/// HGOILRIG (V2): a player at its first spawn (the `HungerGame` points,
/// which have nothing under them in the file but the sea: no deck in the
/// physics BSP, the render data or a world model there) starts on the
/// `Water` volume's top (y = −58 in the file: viscosity 0.1, current
/// (0, 1500, 0)) and floats there for 10 s: its hull's box touches the
/// water (the eye is above it), the current lifts it until the box
/// leaves, gravity takes it back.
#[test]
fn hgoilrig_player_floats() {
    if !keyed() {
        return;
    }
    let Some(mount) = mount() else { return };
    let map = world::load_world(&mount, "hgoilrig").unwrap();
    let (feet, _) = map.spawns[0];
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y * 0.5, movement::ID);
    // The water's top, y = -58 in the file.
    let top = -58.0;
    let mut low = f32::MAX;
    for tick in 1..=(10.0 / combat_arms::game::TICK_INTERVAL) as u32 {
        sim.ticks(1);
        let y = file_y(&sim, p) - mv(&sim, p).half_height;
        low = low.min(y);
        if tick % 100 == 0 {
            eprintln!("hgoilrig: tick {tick}: the feet are at y {y}");
        }
        assert!(y > top - 120.0, "sinking: feet at {y}");
        if tick as f64 * combat_arms::game::TICK_INTERVAL > 7.0 {
            assert!((y - top).abs() < 5.0, "floating at the water's top ({top}): feet at {y}");
            assert!(!mv(&sim, p).eye_free, "the eye above the water");
        }
    }
    eprintln!("hgoilrig: lowest feet {low}");
}

/// HGOILRIG's sea, swimming (movement.md "Liquids and swimming"): a
/// player put deep in the water (eye in it) looking down and holding
/// forward dives; crouching sinks it; let go, the current lifts it.
#[test]
fn hgoilrig_swimming() {
    if !keyed() {
        return;
    }
    let Some(mount) = mount() else { return };
    let map = world::load_world(&mount, "hgoilrig").unwrap();
    let (feet, _) = map.spawns[0];
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let p = sim.spawn_character(feet + Vec3::Y * 0.5, movement::ID);
    sim.ticks(2);
    let s = movement_data().meters_per_unit;
    let at = sim.position(p) / s;
    // 300 u under the top (-58).
    place(&mut sim, p, Vec3::new(at.x, -358.0, -at.z));
    sim.ticks(1);
    assert!(mv(&sim, p).free_movement && mv(&sim, p).in_liquid, "swimming: {:?}", mv(&sim, p));
    let start = file_y(&sim, p);
    sim.intent(p).pitch = -0.8;
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(60);
    let dived = file_y(&sim, p);
    eprintln!("hgoilrig swimming: dived from {start} to {dived}");
    assert!(dived < start - 30.0, "dived: {start} -> {dived}");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.intent(p).pitch = 0.0;
    sim.intent(p).crouch = true;
    sim.ticks(10);
    let m = mv(&sim, p);
    assert!(m.accel.y < 0.0 && m.crouch_rate == 0.0, "crouch sinks: {m:?}");
    // Let go: the viscosity and the current stop the sinking (no vertical
    // limit: it is fast), then lift the player at 18.4 u/s.
    sim.intent(p).crouch = false;
    sim.seconds(3.0);
    let low = file_y(&sim, p);
    sim.seconds(1.0);
    let y = file_y(&sim, p);
    eprintln!("hgoilrig swimming: rising from {low} to {y}");
    assert!((y - low - 18.375).abs() < 1.0, "rising: {low} -> {y}");
}

/// HGOILRIG's ladder `Ladder0` (file box (110, 517.7, −140.4) ..
/// (150, 954.3, −116.4)): a player standing at its foot on the deck,
/// facing +Z in the file and looking up, climbs it from the ground (no
/// jump needed) and gets off on the deck at its top.
#[test]
fn hgoilrig_ladder() {
    if !keyed() {
        return;
    }
    let Some(mount) = mount() else { return };
    let s = movement_data().meters_per_unit;
    let (lo, hi) = (Vec3::new(110.0, 517.7, -140.4), Vec3::new(150.0, 954.3, -116.4));
    let c = (lo + hi) / 2.0;
    let (mut sim, p) = sim_on(&mount, "hgoilrig", file_to_engine(Vec3::new(c.x, lo.y + 60.0, c.z)) * s);
    sim.ticks(5);
    let foot = file_y(&sim, p);
    assert!(mv(&sim, p).on_ladder && mv(&sim, p).on_ground, "at the ladder's foot");
    sim.intent(p).pitch = 1.2;
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(40);
    assert!(file_y(&sim, p) > foot + 20.0, "climbing from the ground: {foot} -> {}", file_y(&sim, p));
    let mut top = f32::MIN;
    for _ in 0..400 {
        sim.ticks(1);
        top = top.max(file_y(&sim, p));
    }
    let m = mv(&sim, p);
    eprintln!("hgoilrig ladder: from y {foot} up to {top}, now at {}", sim.position(p) / s);
    assert!(!m.on_ladder && m.on_ground, "off the ladder, on the deck: {m:?}");
    assert!(file_y(&sim, p) > hi.y, "on the deck above: {}", file_y(&sim, p));
}

/// GRAVITYA's `Gravity01` (PlayerGravity −300, file y −192 .. 128): at
/// rest in the air in it, −300 × 0.015 = −4.5 u/s after a tick. The map's
/// whole-map safety net (`SafteyNet0`, y −192 .. 576) takes the fall
/// damage away: dropped from 400 u over the floor, no damage.
#[test]
fn gravitya_low_gravity() {
    let Some(mount) = mount() else { return };
    if world::find(&mount, "gravitya").is_none() || !keyed() {
        return;
    }
    let s = movement_data().meters_per_unit;
    let (mut sim, p) = sim_on(&mount, "gravitya", file_to_engine(Vec3::new(0.0, 100.0, 0.0)) * s);
    sim.ticks(5);
    let floor = file_y(&sim, p);
    eprintln!("gravitya: the floor's centre at y {floor}");
    place(&mut sim, p, Vec3::new(0.0, floor + 20.0, 0.0));
    sim.ticks(1);
    let v = sim.velocity(p) / s;
    assert!((v.y + 4.5).abs() < 1e-3, "{v}");
    // Dropped from 400 u over the floor at (600, 0): no damage in the net;
    // with the net taken out, the drop's 50 HP.
    for net in [true, false] {
        let at = Vec3::new(600.0, 100.0, 0.0);
        let (mut sim, p) = sim_on(&mount, "gravitya", file_to_engine(at) * s);
        if !net {
            let world = sim.app.world_mut();
            let nets: Vec<Entity> = world
                .query::<(Entity, &CaVolume)>()
                .iter(world)
                .filter(|(_, v)| v.is_safety_net())
                .map(|(e, _)| e)
                .collect();
            assert_eq!(nets.len(), 1);
            world.despawn(nets[0]);
        }
        sim.ticks(5);
        let floor = file_y(&sim, p);
        place(&mut sim, p, Vec3::new(at.x, floor + 400.0, at.z));
        sim.seconds(8.0);
        assert!(mv(&sim, p).on_ground && (file_y(&sim, p) - floor).abs() < 1.0, "landed: {}", file_y(&sim, p));
        let health = sim.app.world().get::<Health>(p).unwrap().current;
        let want = if net { 1.0 } else { 0.5 };
        assert!((health - want).abs() < 1e-3, "net {net}: health {health}");
    }
}

/// TWINHIGH's jump pad `JV08` (file box (−960, 128, −1600) .. (−832, 256,
/// −1472), `Velocity` 800, its `Rotation` pointing up): a player dropped
/// on it is launched by itself, without the jump key, on the tick after
/// it lands: 800 u/s up added, 783.5 after the tick; and again after each
/// landing.
#[test]
fn twinhigh_jump_pad() {
    let Some(mount) = mount() else { return };
    if !keyed() {
        return;
    }
    let s = movement_data().meters_per_unit;
    let (mut sim, p) = sim_on(&mount, "twinhigh", file_to_engine(Vec3::new(-896.0, 200.0, -1536.0)) * s);
    let mut launches = Vec::new();
    for tick in 0..300 {
        let was = mv(&sim, p).in_jump;
        sim.ticks(1);
        if !was && mv(&sim, p).in_jump {
            launches.push((tick, (sim.velocity(p) / s).y));
        }
    }
    eprintln!("twinhigh pad launches (tick, speed): {launches:?}");
    assert!(launches.len() >= 2, "launched, and again after landing: {launches:?}");
    for (_, vy) in &launches {
        assert!((vy - 783.5).abs() < 0.5, "{vy}");
    }
}

fn keyed() -> bool {
    let k = combat_arms::game::mount::can_read("V2");
    if !k {
        eprintln!("skipping: no V2 key in chimera.local.toml");
    }
    k
}

/// world_dat.md Corrections (2026-10-22) 1's census: every damaging
/// volume of every readable map with its `Damage` (the map's, else the
/// class default), `DamageType` and what the player takes per second now
/// (Damage x 0.05 at the 20 Hz step) against before (Damage). Printed;
/// the only check is that no volume's rate is the old one.
#[test]
fn volume_damage_census() {
    let Some(mount) = mount() else { return };
    let mut rows: std::collections::BTreeMap<(String, String), Vec<(String, f32)>> = Default::default();
    let mut read = 0;
    for name in world::map_names(&mount) {
        let Some(path) = world::find(&mount, &name) else { continue };
        let Ok(bytes) = mount.read(&path) else { continue };
        let Ok(w) = World::parse(&bytes) else { continue };
        read += 1;
        for o in &w.objects {
            let Some(defaults) = CaVolume::class_box(&o.class, Vec3::ZERO, Vec3::ZERO) else { continue };
            let damage = match o.get("Damage") {
                Some(Value::Real(r)) => *r,
                _ => defaults.damage,
            };
            if damage <= 0.0 {
                continue;
            }
            let kind = match o.get("DamageType") {
                Some(Value::String(s)) => s.to_ascii_uppercase(),
                _ => defaults.damage_type.clone(),
            };
            rows.entry((o.class.clone(), kind)).or_default().push((name.clone(), damage));
        }
    }
    eprintln!("{read} maps read");
    for ((class, kind), list) in &rows {
        let mut values: Vec<f32> = list.iter().map(|(_, d)| *d).collect();
        values.sort_by(|a, b| a.total_cmp(b));
        values.dedup();
        let maps: std::collections::BTreeSet<&str> = list.iter().map(|(m, _)| m.as_str()).collect();
        eprintln!(
            "{class} [{kind}]: {} in {} maps; Damage values {:?} -> HP/s now {:?} (was the Damage itself)",
            list.len(),
            maps.len(),
            values,
            values.iter().map(|d| d * 0.05).collect::<Vec<_>>()
        );
    }
}
