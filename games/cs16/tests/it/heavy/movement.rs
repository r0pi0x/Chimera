//! CS 1.6's movement on the install's maps, traced through their clip
//! hulls (`goldsrc_engine::hulls`, `cs16:movement`'s `clip_hulls`;
//! docs/plans/active/goldsrc.md, M2): de_dust2's spawns hold a player
//! 1/32 unit over the floor in hull 1; walks from them in every direction
//! never put the box where hull 1 is solid (exactly: no allowance) and
//! climb the map's stairs step by step; ducking walks where only hull 3
//! is free; the maps' ladders (`func_ladder`) are climbed to their tops.
//! Passes with a note without an install.

use bevy::prelude::*;
use chimera_core::core::Velocity;
use chimera_movement::{MapClipHulls, QuakeMovement, to_units};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin},
};
use cs16::{game::movement, harness::Cs16Sim};
use goldsrc_formats::bsp::{Bsp, hull::blocks};

use super::maps::{convert, mount};

const UNIT: f32 = 0.0254;

/// The transform over the feet, units.
const ORIGIN: f32 = 36.0;

fn bsp(mount: &chimera_core::mount::Mount, name: &str) -> Bsp {
    Bsp::parse(&mount.read(&goldsrc_engine::content::maps::map_path(name)).unwrap()).unwrap()
}

fn sim_of(map: MapData) -> Sim {
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map));
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.ticks(5);
    assert!(sim.app.world().get_resource::<MapClipHulls>().is_some(), "the map's clip hulls are offered to the movement");
    sim
}

fn me(sim: &Sim, p: Entity) -> QuakeMovement {
    sim.app.world().get::<QuakeMovement>(p).unwrap().clone()
}

/// Feet, units.
fn feet(sim: &Sim, p: Entity) -> Vec3 {
    to_units(sim.position(p)) - Vec3::Z * ORIGIN
}

/// Whether the player's box is where its hull (1 standing, 3 ducked) is
/// solid in the world.
fn in_solid(b: &Bsp, sim: &Sim, p: Entity) -> bool {
    let f = feet(sim, p);
    let (hull, half) = if me(sim, p).ducked { (3, 18.0) } else { (1, 36.0) };
    blocks(b.contents(0, hull, [f.x as f64, f.y as f64, (f.z + half) as f64]))
}

/// A character with its feet at `at` (engine space).
fn spawn(sim: &mut Sim, at: Vec3) -> Entity {
    sim.spawn_character(at + Vec3::Y * ORIGIN * UNIT, movement::ID)
}

#[test]
fn de_dust2_walks_and_stairs_on_the_hulls() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let b = bsp(&mount, "de_dust2");
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).step_by(3).collect();
    let mut sim = sim_of(map);
    let (mut faults, mut steps, mut walks, mut far) = (Vec::new(), 0, 0, 0.0f32);
    for from in spawns {
        // At rest a spawn's player is 1/32 unit over the floor.
        let p = spawn(&mut sim, from);
        sim.ticks(30);
        let f = feet(&sim, p);
        let centre = [f.x as f64, f.y as f64, (f.z + ORIGIN) as f64];
        assert!(me(&sim, p).on_ground && !in_solid(&b, &sim, p), "spawn {from}: not standing");
        assert!(blocks(b.contents(0, 1, [centre[0], centre[1], centre[2] - 0.05])), "spawn {from}: more than 1/20 unit over the floor");
        sim.app.world_mut().despawn(p);
        for k in 0..8 {
            let p = spawn(&mut sim, from);
            sim.ticks(5);
            sim.intent(p).yaw = k as f32 * std::f32::consts::FRAC_PI_4;
            sim.intent(p).move_axis = Vec2::Y;
            let start = feet(&sim, p);
            let mut last = start;
            for t in 0..600 {
                // Ducked for a stretch of every walk: hull 3.
                sim.intent(p).crouch = (200..320).contains(&t);
                let was_on = me(&sim, p).on_ground;
                sim.ticks(1);
                let at = feet(&sim, p);
                if in_solid(&b, &sim, p) {
                    faults.push(format!("from {from} heading {k} tick {t}: the box is in solid at {at}"));
                    break;
                }
                // A step: up 4 units or more in one command, on the ground
                // before and after.
                if was_on && me(&sim, p).on_ground && at.z - last.z >= 4.0 {
                    steps += 1;
                }
                last = at;
            }
            far = far.max(last.truncate().distance(start.truncate()));
            walks += 1;
            sim.app.world_mut().despawn(p);
        }
    }
    eprintln!("de_dust2: {walks} walks of 6 s, the longest {far:.0} units, {steps} steps climbed, {} faults", faults.len());
    assert!(faults.is_empty(), "{} of {walks} walks ended in solid:\n{}", faults.len(), faults.join("\n"));
    assert!(far > 1000.0, "the longest walk went {far} units");
    assert!(steps >= 10, "only {steps} steps climbed: no stairs walked");
}

/// Every ladder volume of `name` climbed from its foot: how many of them a
/// player placed in front of, facing it and holding forward, climbs to
/// within 24 units of the top (or over it).
fn climb_ladders(name: &str) -> Option<(usize, usize)> {
    let mount = mount()?;
    let map = convert(&mount, name, false);
    let b = bsp(&mount, name);
    let ladders: Vec<(Vec3, Vec3)> = map
        .collision_brushes
        .iter()
        .filter(|l| l.ladder)
        .map(|l| {
            let (a, c) = (to_units(l.min), to_units(l.max));
            (a.min(c), a.max(c))
        })
        .collect();
    let mut sim = sim_of(map);
    let mut climbed = 0;
    for (lo, hi) in &ladders {
        let mid = (*lo + *hi) / 2.0;
        // Stand in front of whichever side is free in hull 1, on the
        // floor under it.
        let mut done = false;
        for (dir, reach) in [
            (Vec3::X, (hi.x - lo.x) / 2.0),
            (Vec3::NEG_X, (hi.x - lo.x) / 2.0),
            (Vec3::Y, (hi.y - lo.y) / 2.0),
            (Vec3::NEG_Y, (hi.y - lo.y) / 2.0),
        ] {
            let stand = mid + dir * (reach + 17.0);
            let start = [stand.x as f64, stand.y as f64, (lo.z + ORIGIN + 8.0) as f64];
            if blocks(b.contents(0, 1, start)) {
                continue;
            }
            let down = b.trace(0, 1, start, [start[0], start[1], start[2] - 64.0]);
            let floor = Vec3::new(stand.x, stand.y, down.end[2] as f32 - ORIGIN);
            let p = spawn(&mut sim, chimera_movement::to_engine(floor));
            sim.ticks(10);
            // Facing the ladder (an intent's yaw 0 looks along +Y), looking up.
            sim.intent(p).yaw = (-dir.y).atan2(-dir.x) - std::f32::consts::FRAC_PI_2;
            sim.intent(p).pitch = 60f32.to_radians();
            sim.intent(p).move_axis = Vec2::Y;
            let mut top = f32::MIN;
            let mut on = false;
            for _ in 0..((hi.z - lo.z) / 200.0 * 100.0) as u64 + 300 {
                sim.ticks(1);
                on |= sim.state(p).on_ladder;
                top = top.max(feet(&sim, p).z);
                assert!(!in_solid(&b, &sim, p), "{name}: in solid climbing the ladder at {mid}");
            }
            sim.app.world_mut().get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
            sim.app.world_mut().despawn(p);
            if on && top >= hi.z - 24.0 {
                done = true;
                break;
            }
        }
        if done {
            climbed += 1;
        } else {
            eprintln!("{name}: the ladder {lo} .. {hi} was not climbed");
        }
    }
    Some((climbed, ladders.len()))
}

#[test]
fn ladders_are_climbed_to_their_tops() {
    // de_dust2 has no ladder of its own; de_nuke, cs_assault and de_train
    // have the stock maps' long ones.
    let mut total = (0, 0);
    for name in ["de_dust2", "de_nuke", "cs_assault", "de_train"] {
        let Some((climbed, ladders)) = climb_ladders(name) else { return };
        eprintln!("{name}: {climbed} of {ladders} ladder volumes climbed");
        total = (total.0 + climbed, total.1 + ladders);
    }
    assert!(total.1 >= 4, "{} ladder volumes in the four maps", total.1);
    assert!(total.0 * 4 >= total.1 * 3, "{} of {} ladders climbed", total.0, total.1);
}
