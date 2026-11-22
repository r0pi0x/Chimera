//! The map logic's overlays (`chimera_drawsound`, `chimera_drawtriggers`)
//! list what a map's sounds and triggers cover: a small map with a hurt
//! trigger, a push trigger and an ambient_generic with a radius.

use bevy::prelude::*;
use crate::stack::{
    games::cs_source::{
        movement::{SourceMovementPlugin, to_engine},
        ui::debug_world::{Shape, shapes},
    },
    harness::{CsSim, Sim},
    logic::Logic,
    map::{MapBrush, MapData, MapEntity, MapHull, MapPlugin},
};

const SCALE: f32 = 0.0254;

fn hull(lo: Vec3, hi: Vec3) -> MapHull {
    let b = MapBrush::from_box(lo, hi);
    let points = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect();
    MapHull { planes: b.planes, points }
}

fn entity(pairs: &[(&str, &str)], hulls: Vec<MapHull>) -> MapEntity {
    MapEntity {
        keyvalues: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        hulls,
        mover: false,
        physics: None,
    }
}

fn sim() -> Sim {
    let (a, b) = (to_engine(Vec3::new(-4096.0, -4096.0, -64.0)), to_engine(Vec3::new(4096.0, 4096.0, 0.0)));
    let data = MapData {
        name: "test:overlays".into(),
        collision_brushes: vec![MapBrush::from_box(a.min(b), a.max(b))],
        entities: vec![
            entity(
                &[("classname", "trigger_hurt"), ("damage", "10"), ("spawnflags", "1")],
                vec![hull(Vec3::new(0.0, 0.0, 0.0), Vec3::new(128.0, 128.0, 64.0))],
            ),
            entity(
                &[("classname", "trigger_push"), ("pushdir", "0 0 90"), ("speed", "300"), ("spawnflags", "1")],
                vec![hull(Vec3::new(1000.0, 0.0, 0.0), Vec3::new(1128.0, 128.0, 64.0))],
            ),
            entity(
                &[
                    ("classname", "ambient_generic"),
                    ("origin", "200 200 32"),
                    ("message", "Ambient.Wind"),
                    ("radius", "500"),
                ],
                vec![],
            ),
        ],
        entity_scale: SCALE,
        ..default()
    };
    let mut sim = Sim::new(CsSim, (MapPlugin::new(data), SourceMovementPlugin));
    sim.ticks(3);
    sim
}

#[test]
fn the_overlays_list_the_triggers_and_sound_radii() {
    let sim = sim();
    let logic = sim.app.world().resource::<Logic>();
    let all = |sound, triggers| shapes(&logic.world, logic.scale, sound, triggers, None);
    assert!(all(0, 0).is_empty(), "both off: nothing");

    let triggers = all(0, 1);
    assert_eq!(triggers.len(), 2, "{triggers:?}");
    assert!(triggers.iter().all(|s| matches!(s, Shape::Cuboid { .. })));
    // The hurt trigger's box is 128 x 128 x 64 units, in metres.
    let Shape::Cuboid { min, max, .. } = &triggers[0] else { unreachable!() };
    let size = *max - *min;
    assert!((size.max_element() - 128.0 * SCALE).abs() < 1e-3, "{size}");
    // Hurt red, push blue: two colours.
    let colors: Vec<_> = triggers
        .iter()
        .map(|s| match s {
            Shape::Cuboid { color, .. } | Shape::Sphere { color, .. } => *color,
        })
        .collect();
    assert_ne!(colors[0], colors[1]);

    let sounds = all(1, 0);
    assert_eq!(sounds.len(), 1, "{sounds:?}");
    let Shape::Sphere { radius, .. } = sounds[0] else { panic!("a sphere") };
    assert!((radius - 500.0 * SCALE).abs() < 1e-4);

    // Both together draw both; `near` at level 1 drops what is far.
    assert_eq!(all(1, 1).len(), 3);
    let far = Vec3::new(5000.0, 0.0, 5000.0);
    assert!(shapes(&logic.world, logic.scale, 1, 1, Some(far)).is_empty());
    assert_eq!(shapes(&logic.world, logic.scale, 2, 2, Some(far)).len(), 3, "2: everything");
}
