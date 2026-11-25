//! Animated textures cost the materials that show them, not every material
//! of the map (feedback b33ceded on dod_flash and afd90ba5 on de_dust2:
//! `animate_textures` in most of the frame spikes). A map's thousands of
//! prop materials, a few of them binding frames of three animations at
//! their own rates: a frame change swaps the frames of those few, a
//! material added later is animated too, and a frame of the system stays
//! under a budget of instructions (`Sim::work`; docs/performance.md,
//! "Per-frame work in a match when nothing changed").

use bevy::prelude::*;
use chimera_present::render::{
    prop_material::PropMaterial,
    texture_anim::{TextureAnims, animate_textures},
    world_material::WorldMaterial,
};
use chimera_world::{
    harness::{Sim, own_process},
    map::{MapData, MapTextureAnim},
};

/// Materials that bind no frame, and that do.
const PLAIN: usize = 4000;
const SHOWING: usize = 12;
const FRAMES: u64 = 300;

/// Instructions a frame of `animate_textures`, the mean over `FRAMES`
/// (a test build): measured 2026-11-25, 9.4k with the index by animation,
/// 197k before (every material looked up on each frame change).
const PER_FRAME: u64 = 50_000;

/// The materials' handles, held as the map's entities hold them.
#[derive(Resource)]
struct Kept(#[allow(dead_code)] Vec<Handle<PropMaterial>>);

fn start() -> (Sim, Vec<Handle<Image>>) {
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>().init_asset::<WorldMaterial>().init_asset::<PropMaterial>().add_systems(Update, animate_textures);
    });
    let world = sim.app.world_mut();
    let textures: Vec<Handle<Image>> = (0..9).map(|_| world.resource_mut::<Assets<Image>>().add(Image::default())).collect();
    let data = MapData {
        texture_anims: vec![
            MapTextureAnim { frames: vec![0, 1, 2], rate: 10.0 },
            MapTextureAnim { frames: vec![3, 4], rate: 15.0 },
            MapTextureAnim { frames: vec![5, 6, 7, 8], rate: 24.0 },
        ],
        ..default()
    };
    world.insert_resource(TextureAnims::new(&data, &textures));
    let mut props = world.resource_mut::<Assets<PropMaterial>>();
    let mut kept: Vec<Handle<PropMaterial>> = (0..PLAIN).map(|_| props.add(PropMaterial::default())).collect();
    for i in 0..SHOWING {
        kept.push(props.add(PropMaterial { base: Some(textures[[0, 3, 5][i % 3]].clone()), ..default() }));
    }
    world.insert_resource(Kept(kept));
    (sim, textures)
}

fn shown(sim: &Sim, h: &Handle<PropMaterial>) -> AssetId<Image> {
    sim.app.world().resource::<Assets<PropMaterial>>().get(h).and_then(|m| m.base.as_ref()).map(|b| b.id()).unwrap_or_default()
}

#[test]
fn a_frame_change_touches_the_materials_showing_it() {
    let (mut sim, textures) = start();
    sim.ticks(2);
    assert_eq!(sim.app.world().resource::<TextureAnims>().bound(), Some((0, SHOWING)));
    // One added later is animated too.
    let late = sim.app.world_mut().resource_mut::<Assets<PropMaterial>>().add(PropMaterial { base: Some(textures[0].clone()), ..default() });
    let mut seen = std::collections::HashSet::new();
    for _ in 0..64 {
        sim.ticks(1);
        seen.insert(shown(&sim, &late));
    }
    assert_eq!(seen.len(), 3, "the late material showed each of its animation's three frames: {seen:?}");
    assert_eq!(sim.app.world().resource::<TextureAnims>().bound(), Some((0, SHOWING + 1)));
}

#[test]
fn a_frame_of_animated_textures_stays_in_its_budget() {
    if !own_process("texture_anim_cost::a_frame_of_animated_textures_stays_in_its_budget") {
        return;
    }
    let (mut sim, _) = start();
    sim.repeatable();
    sim.ticks(2);
    let Some(w) = sim.work(FRAMES) else {
        eprintln!("skipping: no hardware counters");
        return;
    };
    let spent = w.systems.0.iter().filter(|(k, _)| k.ends_with("animate_textures")).map(|(_, v)| v.0).sum::<u64>() / FRAMES;
    eprintln!("animate_textures: {spent} instructions a frame over {FRAMES} frames ({PLAIN} plain materials, {SHOWING} showing frames)");
    assert!(spent < PER_FRAME, "animate_textures costs {spent} instructions a frame (limit {PER_FRAME})");
}
