//! Combat Arms' movement up the stock maps' stairs (specs/combat_arms/
//! movement.md "Slopes and steps": a step up to 30.5 u is climbed without
//! jumping) on the user's install. Two of JUNK_FLEA's flights are a world
//! model its modes show and hide (`QRT_Hide_L`, which Quarantine's start
//! trigger hides): a node of its own, a kinematic body that was taken for
//! a moving object, which is never a step (a Combat Arms player stood at
//! their foot); a world model that never moves is one
//! (`objects::CaFixedNode`). A flight of its world's and two of
//! WAREHOUSE's are checked beside them. Positions are engine metres (the
//! bottom and top treads' centres); `CHIMERA_STAIRS_TRACE=1` prints each
//! climb's steps. Passes with a note without
//! `[games.combat_arms] path` in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::mount::{Mount, config::LocalConfig};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{GAME, INFO, movement, world},
    harness::CaSim,
};

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

/// A straight flight: its bottom and top treads' centres (engine m).
struct Flight {
    what: &'static str,
    bottom: Vec3,
    top: Vec3,
}

/// A Combat Arms player put on each flight's bottom tread walks
/// toward its top: its feet reach the top tread within 4 s.
fn climbs(map: &str, flights: &[Flight]) {
    let Some(mount) = mount() else { return };
    let Ok(data) = world::load_world(&mount, map) else {
        eprintln!("skipping: {map} not readable");
        return;
    };
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(3);
    let mut failed = Vec::new();
    for f in flights {
        let dir = (f.top - f.bottom).with_y(0.0).normalize();
        let p = sim.spawn_character(f.bottom + Vec3::Y * 1.0, movement::ID);
        sim.seconds(0.5);
        let feet = |sim: &Sim| sim.position(p).y + sim.state(p).hull_min.y;
        assert!(
            (feet(&sim) - f.bottom.y).abs() < 0.1,
            "{map} {}: starts on the bottom tread at {}",
            f.what,
            sim.position(p)
        );
        {
            let mut i = sim.intent(p);
            i.yaw = (-dir.x).atan2(-dir.z);
            i.move_axis = Vec2::Y;
        }
        // Until its feet are on the top tread (a flight may lead on and
        // down again), at most 4 s.
        let mut on_top = false;
        for _ in 0..40 {
            sim.seconds(0.1);
            if std::env::var("CHIMERA_STAIRS_TRACE").is_ok() {
                eprintln!("  {:.2} feet {:.2}", sim.position(p), feet(&sim));
            }
            let at = sim.position(p);
            if (feet(&sim) - f.top.y).abs() < 0.1 && (at - f.top).with_y(0.0).length() < 1.0 {
                on_top = true;
                break;
            }
        }
        let (end, at) = (feet(&sim), sim.position(p));
        eprintln!("{map} {}: feet {:.2} (top {:.2}) at {at:.2}", f.what, end, f.top.y);
        if !on_top {
            failed.push((f.what, end, at));
        }
        sim.app.world_mut().despawn(p);
    }
    assert!(failed.is_empty(), "{map}: stopped short of the top: {failed:.2?}");
}

#[test]
fn up_junk_flea_stairs() {
    climbs(
        "junk_flea",
        &[
            Flight {
                what: "QRT_Hide_L's flight at x -1..2 m",
                bottom: Vec3::new(-1.22, 0.32, -4.97),
                top: Vec3::new(2.38, 3.18, -4.97),
            },
            Flight {
                what: "QRT_Hide_L's flight at z 17..13 m",
                bottom: Vec3::new(5.57, 0.32, 16.79),
                top: Vec3::new(5.57, 3.18, 13.19),
            },
            Flight {
                what: "a flight of the world's own",
                bottom: Vec3::new(16.24, 7.46, 19.49),
                top: Vec3::new(16.24, 10.76, 22.40),
            },
        ],
    );
}

#[test]
fn up_warehouse_stairs() {
    climbs(
        "warehouse",
        &[
            Flight {
                what: "a flight of 11 treads",
                bottom: Vec3::new(-14.83, -100.73, 0.32),
                top: Vec3::new(-14.83, -97.85, -3.28),
            },
            Flight {
                what: "a flight of 8 treads",
                bottom: Vec3::new(6.46, -103.82, 34.33),
                top: Vec3::new(8.98, -101.97, 34.33),
            },
        ],
    );
}
