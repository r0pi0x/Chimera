//! A player and dm_lockdown's own physics props, on the install's collision
//! models (the playtest's reports): walking into the bench doesn't throw
//! him in the air, jumping onto the laundry cart doesn't hold him.

use bevy::prelude::*;
use chimera_core::core::{MovementState, Velocity};
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin, PropEntity},
};
use hl2dm::{
    game::{movement, weapons::launch::{U, to_engine, to_source}},
    harness::Hl2Sim,
};

use super::map_stock::{convert, install_mount};

struct Map {
    sim: Sim,
    /// (entity index, model, Source origin).
    props: Vec<(usize, String, Vec3)>,
    plug: usize,
}

fn dm_lockdown() -> Option<Map> {
    let mount = install_mount()?;
    let data = convert(&mount, "dm_lockdown", &MapLoadOptions::lean());
    let props = data
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname().starts_with("prop_physics"))
        .filter_map(|(i, e)| {
            let o: Vec<f32> = e.get("origin")?.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            Some((i, e.get("model")?.to_string(), Vec3::new(o[0], o[1], o[2])))
        })
        .collect();
    let plug = data.entities.iter().position(|e| e.get("targetname") == Some("plug2")).expect("plug2");
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
    sim.set_tick_interval(0.015);
    sim.seconds(2.0);
    Some(Map { sim, props, plug })
}

impl Map {
    fn node(&mut self, index: usize) -> Entity {
        let w = self.sim.app.world_mut();
        w.query::<(Entity, &PropEntity)>().iter(w).find(|(_, p)| p.0 == index).expect("the prop's node").0
    }

    fn at(&self, node: Entity) -> Vec3 {
        to_source(self.sim.app.world().get::<GlobalTransform>(node).unwrap().translation())
    }
}

/// Every step's (feet z, velocity z, on ground) of a walk at `yaw` from
/// `from`.
fn walk(sim: &mut Sim, p: Entity, from: Vec3, yaw: f32, ticks: u32, jump_at: Option<u32>) -> Vec<(Vec3, f32, bool)> {
    {
        let w = sim.app.world_mut();
        w.get_mut::<Transform>(p).unwrap().translation = to_engine(from + Vec3::Z * 36.0);
        w.get_mut::<Velocity>(p).unwrap().0 = Vec3::ZERO;
    }
    sim.ticks(40);
    let mut out = Vec::new();
    for t in 0..ticks {
        {
            let mut i = sim.intent(p);
            i.yaw = yaw;
            i.move_axis = Vec2::Y;
            i.jump = jump_at == Some(t);
        }
        sim.ticks(1);
        let w = sim.app.world();
        let feet = to_source(w.get::<Transform>(p).unwrap().translation) - Vec3::Z * 36.0;
        let v = w.get::<Velocity>(p).unwrap().0.y / U;
        let ground = w.get::<MovementState>(p).unwrap().ground.is_some();
        out.push((feet, v, ground));
    }
    sim.intent(p).move_axis = Vec2::ZERO;
    out
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    let d = to_engine(to - from);
    (-d.x).atan2(-d.z)
}

/// The playtest's report: walking into the bench threw the player up (the
/// step up onto it at 18 units a tick, as a velocity). The real server,
/// walking a bot into a bench and a cart from each side: its vertical speed
/// never over 0 (`probe`: z up to 21 as it climbs).
#[test]
fn walking_into_the_benches_and_carts() {
    let Some(mut m) = dm_lockdown() else { return };
    let targets: Vec<_> = m
        .props
        .iter()
        .filter(|(_, model, _)| model.contains("bench") || model.contains("cart"))
        .cloned()
        .collect();
    assert!(targets.len() >= 8, "{targets:?}");
    let p = m.sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 10000.0)), movement::ID);
    let mut worst = 0.0f32;
    for (index, model, _) in targets {
        let n = m.node(index);
        let at = m.at(n);
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            let from = at + Vec3::new(a.cos(), a.sin(), 0.0) * 96.0;
            let feet = Vec3::new(from.x, from.y, at.z + 6.0);
            let yaw = yaw_to(feet, at);
            let path = walk(&mut m.sim, p, feet, yaw, 90, None);
            let top = path.iter().fold(f32::MIN, |v, s| v.max(s.1));
            eprintln!("{model} #{index} dir {k}: top vz {top:.0}, prop at {:?}", m.at(n));
            worst = worst.max(top);
        }
    }
    // Not 0: our props get under the box as vphysics' don't, but no launch.
    assert!(worst < 400.0, "thrown up at {worst} units/s");
}

/// The playtest's other report: on a cart the player sticks. Dropped into
/// each cart, he walks off it whichever way he goes, as on the real server
/// (the bot landed at z 38 and walked off at once, each way, the cart moving
/// at most 4 units). (Pressed against a cart from the floor after a jump the
/// real bot does hang: not on the ground, under 2 units in 30 ticks; that
/// part is the original's.)
#[test]
fn jumping_onto_the_carts() {
    let Some(mut m) = dm_lockdown() else { return };
    let targets: Vec<_> = m.props.iter().filter(|(_, model, _)| model.contains("cart")).cloned().collect();
    let p = m.sim.spawn_character(to_engine(Vec3::new(0.0, 0.0, 10000.0)), movement::ID);
    let mut walked = 0;
    let mut stuck = Vec::new();
    for (index, model, _) in targets {
        let n = m.node(index);
        for k in 0..4 {
            let at = m.at(n);
            let yaw = k as f32 * std::f32::consts::FRAC_PI_2;
            let path = walk(&mut m.sim, p, at + Vec3::Z * 60.0, yaw, 90, None);
            let last = path[89].0;
            let moved = path[0].0.with_z(0.0).distance(last.with_z(0.0));
            eprintln!("{model} #{index} yaw {k}: landed z {:.1}, {moved:.0} units on, end {last:?}", path[0].0.z);
            walked += 1;
            if moved < 24.0 {
                stuck.push((index, k, moved));
            }
        }
    }
    eprintln!("{walked} walks; stuck {stuck:?}");
    assert!(stuck.len() * 4 <= walked, "stuck on {stuck:?} of {walked}");
}

/// dm_lockdown's own explosive drums (prop data: 120 damage in 256 units;
/// the real server, a drum broken beside a standing bot: 93.97 at 60 units,
/// 66.85 at 120, 30.29 at 200, none at 300, blast + surface damage named by
/// the drum) go off and hurt.
#[test]
fn a_placed_drum_explodes() {
    use chimera_core::core::Health;
    let Some(mut m) = dm_lockdown() else { return };
    let (index, _, _) = m.props.iter().find(|(_, model, _)| model.contains("oildrum001_explosive")).cloned().expect("a drum");
    let n = m.node(index);
    let at = m.at(n);
    assert_eq!(m.props.iter().filter(|(_, model, _)| model.contains("oildrum001_explosive")).count(), 21);
    let p = m.sim.spawn_character(to_engine(at + Vec3::new(120.0, 0.0, 40.0)), movement::ID);
    m.sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 50.0;
    m.sim.ticks(60);
    let feet = to_source(m.sim.app.world().get::<Transform>(p).unwrap().translation) - Vec3::Z * 36.0;
    let centre = m.at(n);
    let before = m.sim.app.world().get::<Health>(p).unwrap().current;
    {
        let mut logic = m.sim.app.world_mut().resource_mut::<source_logic::logic::Logic>();
        let id = logic.world.by_map_index(index).expect("the drum");
        logic.world.queue_direct(id, "Break", source_logic::logic::Value::Void, 0.0, None);
    }
    m.sim.ticks(4);
    let lost = (before - m.sim.app.world().get::<Health>(p).unwrap().current) * 100.0;
    eprintln!("feet {feet:?} drum {centre:?} lost {lost}");
    // (Its neighbours go off with it: more than one blast may reach him.)
    assert!(lost > 40.0, "{lost} points 120 units from it");
}

/// `interior_fence004a` at the first reported spot ("rotated wrongly"): its
/// torn corner is where the map's angles (0 90 -90) put it on the real server
/// too. A bot walking north along x = -3600 stops at y 5522 (the corner); with
/// the roll the other way that corner is up by the ceiling, and it would
/// stop at 5562.
#[test]
fn the_torn_fence_stands_as_on_the_real_server() {
    let Some(mut m) = dm_lockdown() else { return };
    let p = m.sim.spawn_character(to_engine(Vec3::new(-3600.0, 5440.0, 40.0)), movement::ID);
    let yaw = yaw_to(Vec3::new(-3600.0, 5440.0, 0.0), Vec3::new(-3600.0, 5600.0, 0.0));
    let path = walk(&mut m.sim, p, Vec3::new(-3600.0, 5440.0, 0.03), yaw, 80, None);
    let end = path[79].0;
    eprintln!("stopped at {end:?}");
    assert!((end.y - 5522.0).abs() < 6.0, "{end:?}");
}

/// The plug (`plug2`, a func_physbox): it starts on the floor beside the
/// machine, as on the real server (-3800.75 5766.28 7.01, read over RCON at
/// a fresh load); put into the slot's `trigger_teleport` it goes to
/// `plug_destination` (-3786 5773 58) and stays, and the fan starts.
#[test]
fn the_plug_seats_in_its_slot() {
    use avian3d::prelude::{LinearVelocity, Position, Rotation};
    let Some(mut m) = dm_lockdown() else { return };
    let n = m.node(m.plug);
    m.sim.seconds(2.0);
    let at = m.at(n);
    eprintln!("plug rests at {at:?}");
    assert!((at.z - 7.0).abs() < 12.0 && at.distance(Vec3::new(-3800.75, 5766.28, 7.01)) < 40.0, "{at:?}");
    let slot = to_engine(Vec3::new(-3788.5, 5773.0, 58.0));
    {
        let w = m.sim.app.world_mut();
        w.get_mut::<Position>(n).unwrap().0 = slot;
        w.get_mut::<Transform>(n).unwrap().translation = slot;
        w.get_mut::<Rotation>(n).unwrap().0 = Quat::IDENTITY;
        w.get_mut::<LinearVelocity>(n).unwrap().0 = Vec3::ZERO;
    }
    m.sim.seconds(0.5);
    let seated = m.at(n);
    eprintln!("plug seated at {seated:?}");
    assert!(seated.distance(Vec3::new(-3786.0, 5773.0, 58.0)) < 0.5, "{seated:?}");
    let turned = m.sim.app.world().get::<Rotation>(n).unwrap().0;
    assert!(turned.angle_between(Quat::IDENTITY) < 0.01, "{turned:?}");
    assert!(m.sim.app.world().get::<avian3d::prelude::RigidBody>(n).unwrap().is_static(), "pinned");
}
