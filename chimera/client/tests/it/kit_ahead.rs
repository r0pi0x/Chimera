//! The gun in hand ahead of the code-built maps' kit (`console::
//! load_code_map`, a sandbox's start): the kit loads on a thread of its own
//! while the range is played, and a game's view models handed over as its
//! dressing is done (`MapLoadOptions::early`) are added for a view that
//! wants one (`map::add_view_model`), so the gun is drawn when its own
//! game's parts are in, not when every game's are (a tester's cold
//! sandbox showed the stand-in gun 33 s). Here the kit's load hands over
//! game `a`'s view models and then waits until the test lets it go: the
//! wanted one is added while it waits, the one nobody holds is not, and
//! the kit, once in, brings both.

use std::sync::{Condvar, Mutex};

use bevy::prelude::*;
use chimera_client::client::{AppHooks, console};
use chimera_sim::SimBasePlugins;
use chimera_world::{
    harness::Sim,
    map::{
        LateViewModelTextures, MapData, MapLoadOptions, MapMesh, MapModel, MapSimPlugin, MapTexture, MapViewModel,
        ViewAnimator, ViewModels,
    },
};

/// The kit's load waits for this (the test lets it go).
static KIT_GOES: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

fn view_model(key: &str) -> MapViewModel {
    MapViewModel {
        key: key.into(),
        model: MapModel {
            meshes: vec![MapMesh {
                texture: Some(0),
                ..default()
            }],
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

/// Game `a`'s parts: two guns on one texture.
fn parts() -> MapData {
    MapData {
        name: "chimera:range".into(),
        entity_scale: 1.0,
        textures: vec![MapTexture {
            name: "a/gun".into(),
            srgb: true,
            mips: Vec::new(),
            width: 1,
            height: 1,
            rgba8: vec![255; 4],
            bc: None,
        }],
        kit: chimera_world::map::GameKit {
            view_models: vec![view_model("a:gun"), view_model("a:other")],
            ..default()
        },
        ..default()
    }
}

/// The kit's load: `a`'s view models handed over, then a wait for the test.
fn kit(_: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    let parts = parts();
    if let Some(early) = &options.early {
        early.publish("a", &parts, std::time::Duration::ZERO);
    }
    let (goes, told) = &KIT_GOES;
    let go = told.wait_while(goes.lock().unwrap(), |go| !*go).unwrap();
    drop(go);
    Ok(parts)
}

fn keys(sim: &Sim) -> Vec<String> {
    sim.app
        .world()
        .get_resource::<ViewModels>()
        .map(|v| v.0.iter().map(|m| m.key.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn the_gun_in_hand_is_added_before_the_kit_is_in() {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(MapSimPlugin::empty())
            .insert_resource(AppHooks {
                build: "test",
                load_map: kit,
                load_map_file: |id, _, _| Err(format!("{id}: not in tests")),
                map_file_bytes: |_| None,
                unload_greybox: |_| {},
                respawn_greybox: |_| {},
            })
            .add_systems(Update, console::finish_map_load);
    });
    sim.ticks(2);
    assert!(
        !sim.app.world().contains_resource::<chimera_core::tasks::WaitForTasks>(),
        "this test is of the kit on its own thread"
    );
    sim.app.world_mut().spawn(ViewAnimator {
        key: Some("a:gun".into()),
        ..default()
    });
    console::load_code_map(sim.app.world_mut(), "range");
    // The wanted gun comes while the kit's load still waits.
    for _ in 0..2000 {
        if keys(&sim).contains(&"a:gun".to_string()) {
            break;
        }
        sim.app.update();
        chimera_world::harness::let_tasks_run();
    }
    assert_eq!(keys(&sim), ["a:gun"], "only the wanted one, ahead of the kit");
    let late = sim.app.world().resource::<LateViewModelTextures>();
    assert_eq!(late.0["a:gun"][0].name, "a/gun", "with the textures it names");
    let report = console::KitLoads::loading(sim.app.world()).expect("the kit is still loading");
    assert!(report.starts_with("range's kit loading for"), "{report}");
    // The kit, once in, brings every gun.
    *KIT_GOES.0.lock().unwrap() = true;
    KIT_GOES.1.notify_all();
    for _ in 0..2000 {
        if keys(&sim).contains(&"a:other".to_string()) {
            break;
        }
        sim.app.update();
        chimera_world::harness::let_tasks_run();
    }
    let mut all = keys(&sim);
    all.sort();
    assert_eq!(all, ["a:gun", "a:other"]);
    let loads = sim.app.world().resource::<console::KitLoads>();
    let lines: Vec<&str> = loads.0.iter().map(String::as_str).collect();
    assert!(lines.iter().any(|l| l.starts_with("a's view models (2) in")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("a:gun: its view model added ahead of the kit")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("range's kit: 0 bodies, loaded in")), "{lines:?}");
}
