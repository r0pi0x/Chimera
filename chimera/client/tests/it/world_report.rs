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

/// The bundle's `body:` block (`world_report::body_text`; report
/// af42b6c4, "the head is black", named no body or piece): the local
/// player's model, skin and body number, and each piece with its body
/// part, whether the body number shows it, its texture or its flat
/// colour; the shown pieces without a texture listed.
#[test]
fn the_body_lines_name_the_model_and_each_piece() {
    use chimera_world::map::{BodyModel, BodyNumber, BodySkin, CharacterModels, MapCharacterModel};
    let mesh = |material: &str, texture: Option<usize>, body: Option<(u16, u16)>| MapMesh {
        material: material.into(),
        texture,
        body,
        color: [10, 10, 10],
        ..default()
    };
    let model = MapCharacterModel {
        game: Some("game"),
        team: None,
        name: Some("soldier_f".into()),
        class: None,
        model: MapModel {
            meshes: vec![mesh("suit", Some(0), None), mesh("face", None, Some((0, 0))), mesh("helmet", Some(1), Some((0, 1)))],
            body_parts: vec![2],
            mesh_names: vec!["body".into(), "head".into(), "head2".into()],
            ..default()
        },
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: default(),
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let mut world = World::new();
    world.insert_resource(CharacterModels(std::sync::Arc::new(vec![model])));
    world.spawn((LocalPlayer, BodyModel(0), BodySkin(0), BodyNumber(0)));
    let text = world_report::body_text(&mut world);
    assert!(text.contains("body: model 0, name -, skin 0, body number 0; drawn none\n"), "{text}");
    assert!(text.contains("body: soldier_f of game, team -, class -, 3 pieces, 0 bones, parts [2], 0 skins\n"), "{text}");
    assert!(text.contains("body piece 0: body: suit, always, texture #0, not built\n"), "{text}");
    assert!(text.contains("body piece 1: head: face, part 0 choice 0, no texture (flat [10, 10, 10]), not built\n"), "{text}");
    assert!(text.contains("body piece 2: head2: helmet, part 0 choice 1, hidden, texture #1, not drawn\n"), "{text}");
    assert!(text.contains("body untextured (shown): face\n"), "{text}");
}
