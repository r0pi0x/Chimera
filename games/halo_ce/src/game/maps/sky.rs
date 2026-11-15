//! The level's sky, fog and look (specs/halo_ce/scenario.md, "Skies and
//! the sky tag", "How a renderer uses the sky"; shaders.md, "Atmospheric
//! fog"), as far as the readers go, and the seams for what needs the
//! model reader.
//!
//! - Fog (`fog`): the outdoor fog of sky 0 when a cluster of the level
//!   shows a sky (the camera is outdoors there), else its indoor fog;
//!   none without a sky (Chiron TL-34, Rat Race). The game's factor
//!   f = clamp((d - start) / (opaque - start)) x density, d the planar
//!   view depth, blending toward the fog colour by f, is exactly the
//!   engine's linear fog (`MapFog::linear`: f = min(max density,
//!   (d - start) / (end - start))) with end = start + (opaque - start) /
//!   density. A density of 0 or less is 1; an opaque distance of 0 is
//!   no fog; opaque below start is raised to it.
//! - Background (`background`): the fog colour as a flat sky cube (six
//!   4 x 4 faces of one colour, `MapSky`), so the level is not drawn
//!   against black where the sky model will be.
//! - `sky_model`: the seam for the sky model, drawn as a 3D skybox
//!   (`MapSkyCamera`, scale 1024 about the camera, unlit, behind the
//!   world: "How a renderer uses the sky", 2 and 3) once the model reader
//!   exists; `scenery`: the seam for the scenario's scenery placements.
//!   Both only note what is not drawn.
//!
//! Known shortcuts:
//! - The fog colour blends in the engine's linear light, the game's in
//!   gamma; one fog for the whole map (the game switches between the
//!   outdoor and indoor fog by the camera's cluster, easing between them).
//! - The ambient radiosity colours and the sky lights have no place in
//!   `MapLook` (and the game reads them only when baking lightmaps).
//! - No sky model, no lens flares, no scenery (seams above).

use blam_formats::{
    cache::MapFile,
    scenario::{Fog, Scenario, Sky},
    structure_bsp::StructureBsp,
};

use super::space::METERS_PER_UNIT;
use crate::map::{MapData, MapFog, MapSky, MapTexture, MapUvTransform};

/// The engine's fog for a Halo fog (module doc); None when it is off.
pub fn fog(f: &Fog) -> Option<MapFog> {
    if f.opaque_distance <= 0.0 {
        return None;
    }
    let density = if f.max_density <= 0.0 { 1.0 } else { f.max_density.min(1.0) };
    let start = f.start_distance;
    let opaque = f.opaque_distance.max(start);
    // A start equal to the opaque distance is a step: a tiny ramp.
    let span = ((opaque - start) / density).max(1e-3);
    Some(MapFog {
        color: [f.color.r, f.color.g, f.color.b],
        start: start * METERS_PER_UNIT,
        end: (start + span) * METERS_PER_UNIT,
        max_density: density,
        linear: true,
    })
}

/// The level's sky 0, if it reads.
pub fn level_sky(map: &MapFile, scenario: &Scenario, warnings: &mut Vec<String>) -> Option<Sky> {
    let r = scenario.skies.first()?;
    let tag = map.tag(r.sky.id)?;
    match Sky::read(map, tag) {
        Ok(s) => Some(s),
        Err(e) => {
            warnings.push(format!("sky `{}` unreadable: {e}", tag.path));
            None
        }
    }
}

/// Fill the map's fog and background from the sky (module doc).
pub fn add(sky: Option<&Sky>, bsp: &StructureBsp, map: &mut MapData, decode: bool) {
    let Some(sky) = sky else { return };
    let outdoors = bsp.clusters.iter().any(|c| c.sky.is_some());
    let chosen = if outdoors { &sky.outdoor_fog } else { &sky.indoor_fog };
    map.fog = fog(chosen);
    let c = chosen.color;
    background(map, [c.r, c.g, c.b], decode);
}

/// A flat sky cube of colour `rgb` (0..1, sRGB).
pub fn background(map: &mut MapData, rgb: [f32; 3], decode: bool) {
    let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    let texel = [r, g, b, 255];
    map.textures.push(MapTexture {
        name: "<halo sky background>".into(),
        srgb: true,
        mips: Vec::new(),
        width: 4,
        height: 4,
        rgba8: if decode { texel.repeat(16) } else { Vec::new() },
        bc: None,
    });
    let t = map.textures.len() - 1;
    map.sky = Some(MapSky {
        faces: [(t, 0); 6],
        transforms: [MapUvTransform::IDENTITY; 6],
        hdr: None,
    });
}

/// SEAM: the sky's model as a 3D skybox (`MapSkyCamera`: origin at the
/// model's centre, scale 1024, its meshes `skybox` and unlit), as Combat
/// Arms' `sky_camera` adds its sky world model. Needs the model reader
/// (models.md); until then it notes that the model is not drawn.
pub fn sky_model(map: &MapFile, sky: Option<&Sky>, data: &mut MapData) {
    if let Some(model) = sky.map(|s| s.model).filter(|m| !m.id.is_none())
        && let Some(tag) = map.tag(model.id)
    {
        data.warnings.push(format!(
            "sky model `{}` not drawn yet (needs the model reader): a flat background instead",
            tag.path
        ));
    }
}

/// SEAM: the scenario's scenery placements (`Scenario::scenery`, with the
/// scenery palette) as props, once the model reader exists.
pub fn scenery(scenario: &Scenario, data: &mut MapData) {
    if !scenario.scenery.is_empty() {
        data.warnings.push(format!(
            "{} scenery placements not drawn yet (needs the model reader)",
            scenario.scenery.len()
        ));
    }
}
