//! Every stock map plays in the app's simulation (`hl2dm_app::SimPlugin`
//! with `map::MapSimPlugin`, the map loaded as `--map hl2dm:<name>` loads
//! it, lean): a player put at each of its first spawns falls onto the
//! floor and stands there, and runs a step. Passes with a note without
//! the install (`$CHIMERA_HL2DM_ROOT`, Steam, `[games.hl2dm] path`).

use bevy::prelude::*;
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin},
};
use hl2dm::game::{STOCK_MAPS, movement};

#[test]
fn every_stock_map_plays() {
    for name in STOCK_MAPS {
        let id = format!("hl2dm:{name}");
        let data = match hl2dm_app::load_map_with(&id, &MapLoadOptions::lean()) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let spawns: Vec<Vec3> = data.spawns.iter().take(3).map(|s| s.0).collect();
        assert!(!spawns.is_empty(), "{id}: no spawns");
        let mut sim = Sim::new(hl2dm_app::SimPlugin, MapSimPlugin::new(data));
        for (i, at) in spawns.iter().enumerate() {
            let p = sim.spawn_character(*at + Vec3::Y * 0.1, movement::ID);
            sim.seconds(2.0);
            assert!(sim.state(p).on_ground, "{id} spawn {i}: stands");
            let fell = at.y - sim.position(p).y;
            assert!(fell < 1.0, "{id} spawn {i}: fell {fell} m (through the floor?)");
            let before = sim.position(p);
            sim.intent(p).move_axis = Vec2::Y;
            sim.seconds(0.3);
            sim.intent(p).move_axis = Vec2::ZERO;
            assert!(sim.position(p).y > before.y - 1.0, "{id} spawn {i}: still on the floor");
            eprintln!("{id} spawn {i}: stands at {:?}", sim.position(p));
        }
    }
}
