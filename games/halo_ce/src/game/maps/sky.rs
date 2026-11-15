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
//!   4 x 4 faces of one colour, `MapSky`): the 2D sky behind the sky
//!   model (the engine draws it first), and the whole sky of a map whose
//!   sky has no model (Chill Out) or that has none (Chiron TL-34, Rat Race).
//! - `sky_model`: the sky's model as a 3D skybox ("How a renderer uses the
//!   sky", 2 and 3): the model, authored about 1024 times the world's
//!   size, is baked at 1/1024 (`sky_point`) into `skybox` meshes (unlit,
//!   their shaders' own looks) and `MapSkyCamera` has origin 0 and scale
//!   1024: the sky camera sits at `eye / 1024`, so the miniature stays
//!   around the player and shifts by 1/1024 of his distance from the
//!   origin, as the game's does, and does not turn with the view.
//! - `prop_light`: the one light scenery is lit with (`objects`).
//!
//! Known shortcuts:
//! - The fog colour blends in the engine's linear light, the game's in
//!   gamma; one fog for the whole map (the game switches between the
//!   outdoor and indoor fog by the camera's cluster, easing between them).
//! - The ambient radiosity colours and the sky lights have no place in
//!   `MapLook` (and the game reads them only when baking lightmaps).
//! - No lens flares; the sky is drawn by the engine's sky camera (its own
//!   depth, sorting and projection), not before everything else with no
//!   depth test; the sky's lights and ambient colours light nothing but
//!   the scenery's probe (`prop_light`).

use bevy::math::Vec3;
use blam_formats::{
    cache::MapFile,
    model::Model,
    scenario::{Fog, Scenario, Sky},
    structure_bsp::StructureBsp,
};

use super::{
    objects::{self, Dest, Pending},
    space::{self, METERS_PER_UNIT},
};
use crate::map::{LightProbe, MapData, MapFog, MapSky, MapSkyCamera, MapTexture, MapUvTransform};

/// The engine's fog for a Halo fog (module doc); None when it is off.
pub fn fog(f: &Fog) -> Option<MapFog> {
    if f.opaque_distance <= 0.0 {
        return None;
    }
    let density = if f.max_density <= 0.0 {
        1.0
    } else {
        f.max_density.min(1.0)
    };
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

/// The sky model's miniature: the factor the model is authored larger than
/// the world (scenario.md, "How a renderer uses the sky", 3: the game
/// draws it scaled by 1/1024 about the camera). The meshes are baked
/// at 1/`SCALE` of their authored size and `MapSkyCamera::scale` is
/// `SCALE`, so the engine's sky camera at `eye / SCALE` sees them as the
/// game does: a sky point `m` (world units) is seen in the direction
/// `m - eye` from the eye, up to the length 1/1024.
pub const SCALE: f32 = 1024.0;

/// A sky model's point (authored world units) in the sky camera's space:
/// metres at 1/`SCALE`.
pub fn sky_point(p: [f32; 3]) -> bevy::math::Vec3 {
    space::point(p) / SCALE
}

/// The sky's model as a 3D skybox (module doc): a `MapSkyCamera` with its
/// origin at the model's origin and scale `SCALE`, the model's meshes
/// (`skybox`, unlit, waiting for their looks) returned. A sky without a
/// model (Chill Out's) or without a sky adds nothing.
pub fn sky_model(file: &MapFile, sky: Option<&Sky>, data: &mut MapData) -> Vec<Pending> {
    let Some(model) = sky.map(|s| s.model).filter(|m| !m.id.is_none()) else {
        return Vec::new();
    };
    let Some(tag) = file.tag(model.id) else {
        data.warnings
            .push("sky model: no such tag, a flat background instead".into());
        return Vec::new();
    };
    let model = match Model::read(file, tag) {
        Ok(m) => m,
        Err(e) => {
            data.warnings.push(format!(
                "sky model `{}` unreadable, a flat background instead: {e}",
                tag.path
            ));
            return Vec::new();
        }
    };
    let meshes = objects::model_meshes(file, &model, &sky_point);
    if meshes.is_empty() {
        data.warnings.push(format!(
            "sky model `{}` has no drawable parts, a flat background instead",
            tag.path
        ));
        return Vec::new();
    }
    data.sky_camera = Some(MapSkyCamera {
        origin: Vec3::ZERO,
        scale: SCALE,
        fog: None,
    });
    meshes
        .into_iter()
        .map(|(mut mesh, shader)| {
            mesh.skybox = true;
            Pending {
                mesh,
                shader,
                lit: false,
                dest: Dest::Sky,
            }
        })
        .collect()
}

/// The light every scenery object is lit with (`objects`, "Known
/// shortcuts"): the engine's probe, a neutral sky-lit ambient (brighter
/// above, darker below) and the sky's first light as a directional light
/// when there is one, in lightmap units (the level's light is clamped
/// 0..1 at scale 1).
pub fn prop_light(sky: Option<&Sky>) -> LightProbe {
    // Cube order: +X, -X, +Y, -Y, +Z, -Z.
    let cube = [0.50, 0.50, 0.65, 0.30, 0.50, 0.50].map(Vec3::splat);
    let lights = sky
        .and_then(|s| s.lights.first())
        .map(|l| {
            let dir = space::direction(l.direction()).normalize_or_zero();
            let c = l.color;
            (dir, Vec3::new(c.r, c.g, c.b) * 0.4)
        })
        .into_iter()
        .collect();
    LightProbe { cube, lights }
}
