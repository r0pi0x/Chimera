//! cs_militia's walls stop the player where CS:S's do (headless, from a
//! real install; skipped without one).

use avian3d::prelude::Position;
use bevy::prelude::*;
use crate::stack::{
    core::Velocity,
    games::{
        self, cs_source,
        cs_source::movement::{self, SourceMovement, SourceMovementPlugin},
    },
    harness::{CsSim, Sim},
    map::{MapData, MapPlugin},
    mount::config::LocalConfig,
};

const M: f32 = 0.0254;

/// Source units (Z up) to engine (meters, Y up).
fn src(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, -p.y) * M
}

fn load(name: &str) -> Option<MapData> {
    LocalConfig::load().ok()?.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())?;
    if !::cs_source::game::INFO.mount().is_some_and(|m| m.contains(&format!("maps/{name}.bsp"))) {
        eprintln!("skipping {name}: not installed");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// Walk from `feet` (Source units) toward Source direction `dir` for
/// `seconds`; the feet's Source position afterwards.
fn walk(map: &MapData, feet: Vec3, dir: Vec2, seconds: f64) -> Vec3 {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map.clone()), SourceMovementPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    // The character's transform is 36 units above its feet.
    let at = src(feet + Vec3::Z * 36.0);
    let p = sim.spawn_character(at, movement::ID);
    {
        let w = sim.app.world_mut();
        let mut e = w.entity_mut(p);
        e.get_mut::<Transform>().unwrap().translation = at;
        if let Some(mut pos) = e.get_mut::<Position>() {
            pos.0 = at;
        }
        e.get_mut::<Velocity>().unwrap().0 = Vec3::ZERO;
        *e.get_mut::<SourceMovement>().unwrap() = SourceMovement::default();
    }
    sim.ticks(16);
    let facing = src(dir.extend(0.0));
    {
        let mut i = sim.intent(p);
        // Intent yaw 0 looks down engine -Z.
        i.yaw = (-facing.x).atan2(-facing.z);
        i.move_axis = Vec2::Y;
    }
    sim.seconds(seconds);
    let e = sim.position(p) / M;
    Vec3::new(e.x, -e.z, e.y - 36.0)
}

/// The concrete wall to the right of a playtest's spot by the back porch
/// (`setpos -69.83 256.39 -15.97; setang 12.54 335.27 0`, the feet on the
/// porch floor): a world brush (x -192..136, y 80..216, z -16..80, a
/// sloped face toward the porch) whose top is a dirt displacement's base.
/// CS:S's traces start solid inside it and cross it from its face (y 184.4
/// at z 60) to y 80; it lost its collision when brushes carrying a
/// displacement were dropped. Walking into it, straight or along the
/// view's right, stops at its face (y 216 at the floor).
#[test]
fn wall_under_a_displacement_stops_the_player() {
    let Some(map) = load("cs_militia") else { return };
    let inside = src(Vec3::new(-28.0, 138.0, 32.0));
    assert!(
        map.collision_brushes
            .iter()
            .any(|b| b.planes.iter().all(|(n, d)| n.dot(inside) <= *d)),
        "no collision brush inside the wall"
    );
    let start = Vec3::new(-69.83, 256.39, -15.97);
    let right = (335.27f32 - 90.0).to_radians();
    for dir in [Vec2::new(0.0, -1.0), Vec2::new(right.cos(), right.sin())] {
        let end = walk(&map, start, dir, 2.0);
        assert!(end.y > 216.0, "walking {dir:?} from {start:?} ended at {end:?}: through the wall");
        // Pressed against the slope the feet ride 2 units up, as CS:S's
        // (movecmp `militia_porch_wall`: y 231.20, z -14.00 both).
        assert!(end.z < start.z + 3.0, "walking {dir:?} ended at {end:?}: over the wall");
    }
}
