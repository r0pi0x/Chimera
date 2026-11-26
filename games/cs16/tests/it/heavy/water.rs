//! CS 1.6's maps' liquids as water volumes (`goldsrc_engine::content::
//! water`; specs/goldsrc/bsp.md "Contents", entities.md "func_water") on
//! the install: every liquid brush entity of the 25 stock maps (the 50
//! `func_water`s and `cs_italy`'s `func_illusionary` `*13`, all `skin`
//! −3) and every liquid leaf of the world gives volumes; a player put in
//! as_oilrig's sea swims by CS 1.6's movement (specs/cs16/movement.md,
//! "Water"). Most stock water is shallow: de_aztec's river is 16 units
//! deep (a wade, water level 1).
//! Passes with a note without an install.

use bevy::prelude::*;
use chimera_movement::{QuakeMovement, to_units};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use cs16::{game::movement, harness::Cs16Sim};
use goldsrc_engine::content::{entities::model_of, maps as gmaps, to_engine, water};
use goldsrc_formats::bsp::{Bsp, hull::liquid, solids::regions_where};

use super::maps::{convert, mount};

#[test]
fn every_liquid_of_the_stock_maps_is_water() {
    let Some(mount) = mount() else { return };
    let (mut entities, mut leaves) = (0, 0);
    let mut wet = Vec::new();
    // CS's own maps (the mount lists Half-Life's too, through `valve`).
    let stock = gmaps::map_names(&mount)
        .into_iter()
        .filter(|n| ["cs_", "de_", "as_"].iter().any(|p| n.starts_with(p)));
    for name in stock {
        let b = Bsp::parse(&mount.read(&gmaps::map_path(&name)).unwrap()).unwrap();
        let world = regions_where(&b, 0, 0, liquid).len();
        let liquids: Vec<_> = b
            .entities()
            .into_iter()
            .filter(|e| water::entity_liquid(e).is_some())
            .collect();
        let map = convert(&mount, &name, false);
        let want_min = world + liquids.len();
        assert!(
            map.water.len() >= want_min,
            "{name}: {} volumes, want at least {want_min}",
            map.water.len()
        );
        // Each liquid entity's volumes hold its model's centre.
        for e in &liquids {
            let m = &b.models[model_of(e).unwrap()];
            let c = [0, 1, 2].map(|a| (m.mins[a] + m.maxs[a]) / 2.0);
            assert!(
                map.water.iter().any(|w| w.brush.contains(to_engine(c))),
                "{name}: {} {:?} centre {c:?} in no volume",
                e.classname(),
                e.get("model")
            );
        }
        assert!(map.water_movers.is_empty(), "{name}: no stock func_water is named");
        entities += liquids.len();
        leaves += world;
        if !map.water.is_empty() {
            let deepest = map
                .water
                .iter()
                .map(|w| (to_units(w.brush.max).z - to_units(w.brush.min).z).abs())
                .fold(0.0f32, f32::max);
            wet.push(format!("{name} {} (deepest {deepest:.0})", map.water.len()));
        }
    }
    eprintln!("liquid entities {entities}, world liquid regions {leaves}; maps with water: {wet:?}");
    assert_eq!(entities, 51, "the 50 func_water and cs_italy's func_illusionary");
}

/// The standing box's centre over its feet, units.
const ORIGIN: f32 = 36.0;

#[test]
fn a_player_swims_in_as_oilrigs_sea() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "as_oilrig", false);
    // The deepest volume (units: z up).
    let (lo, hi) = map
        .water
        .iter()
        .map(|w| (to_units(w.brush.min), to_units(w.brush.max)))
        .map(|(a, b)| (a.min(b), a.max(b)))
        .max_by(|a, b| (a.1.z - a.0.z).total_cmp(&(b.1.z - b.0.z)))
        .expect("as_oilrig has water");
    let depth = hi.z - lo.z;
    assert!(depth > 100.0, "deepest volume {depth} units");
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map));
    sim.set_tick_interval(cs16::game::TICK_INTERVAL);
    sim.ticks(5);
    // Feet 80 units under the surface, over the volume's middle.
    let c = (lo + hi) / 2.0;
    let feet = Vec3::new(c.x, c.y, hi.z - 80.0);
    let p = sim.spawn_character(chimera_movement::to_engine(feet + Vec3::Z * ORIGIN), movement::ID);
    sim.ticks(1);
    let me = |sim: &Sim| sim.app.world().get::<QuakeMovement>(p).unwrap().clone();
    assert_eq!(me(&sim).water_level, 3, "under water to the eye");
    // No input: no gravity, the sink's 60 x 0.8 = 48 units/s down at most.
    sim.ticks(100);
    let vz = to_units(sim.velocity(p)).z;
    assert!(
        me(&sim).water_level >= 2 && (-48.5..0.0).contains(&vz),
        "sinking at {vz}"
    );
    // Jump held: up at 100.
    sim.intent(p).jump = true;
    let z0 = to_units(sim.position(p)).z;
    sim.ticks(20);
    let (vz, z1) = (to_units(sim.velocity(p)).z, to_units(sim.position(p)).z);
    assert!(vz > 50.0 && z1 > z0 + 10.0, "rising at {vz}, {z0} -> {z1}");
}
