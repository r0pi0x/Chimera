//! A Halo map's sounds handed over at its conversion (specs/halo_ce/
//! sounds.md "Sound environment, background sound, sound scenery";
//! structure_bsp.md "background sound palette", "sound environment
//! palette", clusters and leaves; collision.md's point test; scenario.md
//! sound scenery): the kit's sounds (`audio::kit`: the projectiles' and
//! weapons' looping sounds, the footsteps) and the level's part
//! (`audio::level::LevelAudio`): the background sound palette as looping
//! sounds, the sound environment palette as values, each cluster's pair of
//! entries as a zone, the collision BSP's 3D tree pruned to the zones
//! (`audio::level::prune`), every sound scenery placement's looping sound
//! attachments at its position. All of it joins the map through
//! `audio::library::add` (clips and entries in its sounds, the library in
//! a map entity), so the converted-map cache keeps it.

use blam_formats::{
    cache::MapFile,
    collision::LevelCollision,
    object::SoundScenery,
    scenario::Scenario,
    sound::environment::SoundEnvironment,
    structure_bsp::StructureBsp,
};

use super::super::audio::{
    self,
    level::{Bsp, Environment, LevelAudio, SceneryLoop, prune, zone_side},
    library::Compiler,
};
use crate::map::MapData;

/// The level's part of the library (module doc), compiled into `c`.
pub fn level(c: &mut Compiler, scenario: &Scenario, bsp: &StructureBsp, collision: &LevelCollision) -> LevelAudio {
    let file: &MapFile = c.file;
    let backgrounds = bsp
        .background_sound_palette
        .iter()
        .map(|e| c.looping(&e.tag).unwrap_or_default())
        .collect();
    let environments = bsp
        .sound_environment_palette
        .iter()
        .map(|e| {
            let tag = file.tag(e.tag.id);
            let env = tag.and_then(|t| SoundEnvironment::read(file, t).ok());
            match (tag, env) {
                (Some(t), Some(v)) => Environment {
                    name: t.path.to_ascii_lowercase(),
                    room_intensity: v.room_intensity,
                    room_intensity_hf: v.room_intensity_hf,
                    decay_time: v.decay_time,
                    decay_hf_ratio: v.decay_hf_ratio,
                    reflections_intensity: v.reflections_intensity,
                    reflections_delay: v.reflections_delay,
                    reverb_intensity: v.reverb_intensity,
                    reverb_delay: v.reverb_delay,
                    diffusion: v.diffusion,
                    density: v.density,
                    hf_reference: v.hf_reference,
                },
                _ => Environment::default(),
            }
        })
        .collect();
    // Each cluster's zone.
    let pair = |o: Option<u16>| o.map_or(-1, i32::from);
    let mut zones: Vec<[i32; 2]> = Vec::new();
    let cluster_zone: Vec<usize> = bsp
        .clusters
        .iter()
        .map(|cl| {
            let z = [pair(cl.background_sound), pair(cl.sound_environment)];
            zones.iter().position(|q| *q == z).unwrap_or_else(|| {
                zones.push(z);
                zones.len() - 1
            })
        })
        .collect();
    let tree = &collision.bsp;
    let node = |n: u32| {
        let node = tree.nodes.get(n as usize)?;
        let (i, flip) = blam_formats::collision::split_plane(node.plane);
        let p = blam_formats::collision::flipped(*tree.planes.get(i)?, flip);
        Some(([p.normal[0], p.normal[1], p.normal[2], p.d], node.back, node.front))
    };
    let leaf = |l: u32| {
        bsp.leaves
            .get(l as usize)
            .and_then(|leaf| leaf.cluster)
            .and_then(|c| cluster_zone.get(usize::from(c)))
            .map_or(-1, |z| zone_side(*z))
    };
    let (nodes, root) = if zones.len() <= 1 {
        (Vec::new(), if zones.is_empty() { -1 } else { zone_side(0) })
    } else {
        prune(&Bsp { node: &node, leaf: &leaf, nodes: tree.nodes.len() })
    };
    let mut scenery = Vec::new();
    for s in &scenario.sound_scenery {
        let Ok(i) = usize::try_from(s.header.palette_index) else { continue };
        let Some(entry) = scenario.sound_scenery_palette.get(i) else { continue };
        let Some(tag) = file.tag(entry.object.id) else { continue };
        let Ok(object) = SoundScenery::read(file, tag) else { continue };
        let at = super::space::point(s.header.position);
        for sound in c.attached_loops(&object.object.attachments) {
            scenery.push(SceneryLoop { at: at.to_array(), sound });
        }
    }
    LevelAudio { backgrounds, environments, zones, tree: nodes, root, clusters: bsp.clusters.len(), scenery }
}

/// The map's sounds (module doc) added to `map`.
pub fn add(file: &MapFile, scenario: &Scenario, bsp: &StructureBsp, collision: &LevelCollision, map: &mut MapData) {
    let mut c = Compiler::new(file);
    audio::kit(&mut c);
    let level = level(&mut c, scenario, bsp, collision);
    c.set.library.level = Some(level);
    audio::library::add(map, &c.set);
}
