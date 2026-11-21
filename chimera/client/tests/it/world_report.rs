//! The F10 bundle's hand lines (`client::world_report::hand_text`): the
//! shown view model's key, pieces and the pieces without a texture, so a
//! report of grey or missing arms (feedback 42750262, e23eba2a) names
//! them.

use bevy::prelude::*;
use chimera_client::client::world_report;
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::Inventory;
use chimera_world::map::{MapMesh, MapModel, MapViewModel, ViewAnimator, ViewModels};

fn view_model(key: &str) -> MapViewModel {
    let mesh = |material: &str, texture: Option<usize>| MapMesh {
        material: material.into(),
        texture,
        ..default()
    };
    MapViewModel {
        key: key.into(),
        model: MapModel {
            meshes: vec![mesh("gun", Some(0)), mesh("hands", None), mesh("hands", None)],
            ..default()
        },
        bones: Vec::new(),
        root: default(),
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: default(),
    }
}

#[test]
fn the_hand_lines_name_the_view_models_untextured_pieces() {
    let mut world = World::new();
    world.insert_resource(ViewModels::of([view_model("game:claws#h0_20_0")]));
    world.spawn((
        LocalPlayer,
        Inventory::default(),
        ViewAnimator {
            key: Some("game:claws#h0_20_0".into()),
            ..default()
        },
    ));
    let text = world_report::hand_text(&mut world);
    assert!(
        text.contains("view model: game:claws#h0_20_0, 3 pieces, 0 bones; untextured: hands\n"),
        "{text}"
    );
    // A key not loaded says so.
    let mut q = world.query::<&mut ViewAnimator>();
    q.single_mut(&mut world).unwrap().key = Some("game:other".into());
    assert!(world_report::hand_text(&mut world).contains("view model: game:other, not loaded\n"));
}
