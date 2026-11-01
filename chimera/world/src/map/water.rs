//! Water surfaces for any game (Source's Water shader is the model,
//! specs/cs_source/water.md), what the simulation and the drawing share:
//! the games' `MapWaterMaterial`, the spawned surfaces (`WaterSurface`)
//! and the water that moves with its brush entity (`move_water`), where
//! the eye is relative to the water (`WaterView`: the drawing writes it,
//! hearing reads it), the reflection camera's markers and the render
//! layers. `render::water` draws them. Units: meters, Y up.
//!
//! Water for any game: `MapWaterMaterial` from the game; `WaterSurface` bounds
//! and `MapWater` volumes following their brush entity (`move_water`);
//! `WaterView`; `WaterReflectionCamera`, `ReflectionClusters`; `WATER_LAYER`,
//! `REFLECT_LAYER`.

use bevy::{camera::visibility::RenderLayers, prelude::*};

/// Render layer of water surfaces: only main views draw them (never the
/// reflection camera). Main cameras get it while the map has water.
pub const WATER_LAYER: usize = 28;
/// Render layer of world brushes as the reflection sees them (Source's
/// "reflect world": no models). Entities stay on layer 0.
pub const REFLECT_LAYER: usize = 27;

/// How a water surface looks (Source Water material parameters after the
/// game's defaults and quirks are applied).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapWaterMaterial {
    /// The game's material name.
    pub name: String,
    /// The normal map's frames (indices into `MapData::textures`, linear),
    /// all the same size, shown at `frame_rate` per second (hard switch).
    pub normal_frames: Vec<usize>,
    pub frame_rate: f32,
    /// The first normal layer's scroll: texture repeats per second (the
    /// translation wraps to 0..1).
    pub bump_scroll: Vec2,
    /// Three-layer normals when `scroll1.x` is not zero (spec section 4).
    pub scroll1: Vec2,
    pub scroll2: Vec2,
    /// Shows what is below (refraction), distorted by `refract_amount`
    /// (screen uv per unit of normal xy).
    pub refract: bool,
    pub refract_amount: f32,
    /// `$blurrefract`: the refraction is a 5x5 box blur (0.005 uv steps),
    /// tinted by `refract_tint`, and its distortion isn't faded by the fog
    /// alpha (spec sections 5 and 6.2).
    pub blur_refract: bool,
    /// `$refracttint`, gamma 0..1: applied only with `blur_refract` (the
    /// shader's quirk).
    pub refract_tint: [f32; 3],
    /// Planar reflection (a mirrored camera), distorted by `reflect_amount`.
    pub reflect: bool,
    /// The reflection also shows models (props, characters), not just the
    /// world.
    pub reflect_entities: bool,
    pub reflect_amount: f32,
    /// Gamma 0..1.
    pub reflect_tint: [f32; 3],
    /// Water fog colour, gamma 0..1.
    pub fog_color: [f32; 3],
    /// Water fog range, meters.
    pub fog_start: f32,
    pub fog_end: f32,
    /// A top surface (fog from depth below); false: seen from below.
    pub above_water: bool,
    /// Never refraction or reflection: only the cubemap, opaque.
    pub force_cheap: bool,
    /// Planar reflection allowed whatever `WaterSettings::force_expensive`
    /// says (Source's `$forceexpensive`, on unless the material turns it
    /// off).
    pub force_expensive: bool,
    /// Cubemap reflection (the cheap pass), when the surface has one.
    pub envmap: Option<WaterEnvmap>,
    /// The cheap pass's distance ramp, meters: refraction near, cubemap
    /// from `cheap_end` on.
    pub cheap_start: f32,
    pub cheap_end: f32,
    /// None: Fresnel weights the cubemap; Some(f): a fixed weight.
    pub fixed_reflect_weight: Option<f32>,
    /// What the surface looks like from below (the game puts it on the
    /// downward faces).
    pub bottom: Option<Box<MapWaterMaterial>>,
    /// A full-screen warp drawn while the eye is in this water, after the
    /// view models.
    pub underwater_overlay: Option<MapScreenWarp>,
    /// Not a water shader: the map draws the surface with its own material
    /// (de_nuke's additive pool) and only the volume's fog applies (`fog_*`,
    /// `above_water`; nothing else is read). Under water its range fog
    /// covers the whole view, with no refraction or intersection view.
    pub fog_only: bool,
}

/// A full-screen refraction of the finished frame (Source's Refract shader
/// as an overlay): the screen sampled at an offset from a scrolling normal
/// map, blurred and tinted.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapScreenWarp {
    /// The game's material name.
    pub name: String,
    /// The normal map's frames (indices into `MapData::textures`, linear),
    /// shown at `frame_rate` per second.
    pub normal_frames: Vec<usize>,
    pub frame_rate: f32,
    /// Screen uv offset per unit of normal xy.
    pub refract_amount: f32,
    /// Multiplies the refracted colour (gamma 0..1).
    pub tint: [f32; 3],
    /// Blur of the refracted screen (`$bluramount`): 0 none, n a 5 x 5
    /// box of taps n pixels apart.
    pub blur: f32,
    /// Normal-map repeats across the screen (u, v), and its scroll in
    /// repeats per second.
    pub scale: Vec2,
    pub scroll: Vec2,
}

impl Default for MapWaterMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            normal_frames: Vec::new(),
            frame_rate: 0.0,
            bump_scroll: Vec2::ZERO,
            scroll1: Vec2::ZERO,
            scroll2: Vec2::ZERO,
            refract: true,
            refract_amount: 0.0,
            blur_refract: false,
            refract_tint: [1.0; 3],
            reflect: false,
            reflect_entities: false,
            reflect_amount: 0.8,
            reflect_tint: [1.0; 3],
            fog_color: [1.0, 0.0, 0.0],
            fog_start: 0.0,
            fog_end: 0.0,
            above_water: true,
            force_cheap: false,
            force_expensive: true,
            envmap: None,
            cheap_start: 0.0,
            cheap_end: 0.0,
            fixed_reflect_weight: None,
            bottom: None,
            underwater_overlay: None,
            fog_only: false,
        }
    }
}

/// Which cubemap the cheap pass reflects.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WaterEnvmap {
    /// Index into `MapData::cubemaps`.
    Baked(usize),
    /// The cubemap sample nearest to the surface.
    Nearest,
}

/// A spawned water surface (one per water material).
#[derive(Component, Clone, Debug)]
pub struct WaterSurface {
    /// Index into the map's water materials.
    pub material: usize,
    /// The surface's bounds (meters).
    pub min: Vec3,
    pub max: Vec3,
    /// The surface seen from below (the bottom material).
    pub below: bool,
    /// Part of the 3D skybox (drawn by the sky camera, cubemap only).
    pub skybox: bool,
}

/// A water surface that moves with its brush entity's node: its bounds in
/// the node's frame (`WaterSurface`'s follow the node: `move_water`).
#[derive(Component, Clone, Copy, Debug)]
pub struct MovingWaterSurface {
    pub min: Vec3,
    pub max: Vec3,
}

/// The map's water volumes that move with a brush entity
/// (`MapData::water_movers`): each volume's index, its entity's, its brush
/// where the map placed it and the entity's placed transform.
#[derive(Resource, Clone, Debug, Default)]
pub struct WaterMovers(pub Vec<(usize, usize, crate::map::MapBrush, Transform)>);

/// Moving water (func_water_analog, water parented to a mover): each
/// volume follows its entity's node, as placed relative to where the map
/// put it (planes and bounds turned and moved), and each surface's world
/// bounds follow its node.
#[allow(clippy::type_complexity)]
pub fn move_water(
    movers: Option<Res<WaterMovers>>,
    water: Option<ResMut<crate::core::MapWater>>,
    nodes: Query<(&super::MapBrushEntity, &GlobalTransform)>,
    mut surfaces: Query<(&MovingWaterSurface, &mut WaterSurface, &GlobalTransform)>,
) {
    for (m, mut s, g) in &mut surfaces {
        let (min, max) = transformed_bounds(&g.compute_transform(), m.min, m.max);
        if s.min != min || s.max != max {
            s.min = min;
            s.max = max;
        }
    }
    let (Some(movers), Some(mut water)) = (movers, water) else {
        return;
    };
    if movers.0.is_empty() {
        return;
    }
    let by_entity: std::collections::HashMap<usize, Transform> =
        nodes.iter().map(|(b, g)| (b.0, g.compute_transform())).collect();
    for (volume, entity, home_brush, home) in &movers.0 {
        let Some(now) = by_entity.get(entity) else { continue };
        let moved = Transform::from_matrix(now.to_matrix() * home.to_matrix().inverse());
        let brush = place_brush(home_brush, &moved);
        if let Some(v) = water.bypass_change_detection().0.get_mut(*volume)
            && (v.brush.min != brush.min || v.brush.max != brush.max || v.brush.planes != brush.planes)
        {
            v.brush = brush;
            water.set_changed();
        }
    }
}

/// A box's bounds after a transform.
fn transformed_bounds(t: &Transform, min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    (0..8)
        .map(|k| {
            t.transform_point(Vec3::new(
                if k & 1 == 0 { min.x } else { max.x },
                if k & 2 == 0 { min.y } else { max.y },
                if k & 4 == 0 { min.z } else { max.z },
            ))
        })
        .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(p), b.max(p)))
}

/// A brush moved by a rigid transform: each plane n.p = d becomes
/// (R n).p = d + (R n).t.
pub fn place_brush(b: &crate::map::MapBrush, t: &Transform) -> crate::map::MapBrush {
    let (min, max) = transformed_bounds(t, b.min, b.max);
    crate::map::MapBrush {
        planes: b
            .planes
            .iter()
            .map(|(n, d)| {
                let n = t.rotation * *n;
                (n, d + n.dot(t.translation))
            })
            .collect(),
        min,
        max,
        ladder: b.ladder,
        surface: b.surface.clone(),
    }
}

/// The reflection camera.
#[derive(Component)]
pub struct WaterReflectionCamera;

/// The layers a world brush mesh is drawn on (main views and the
/// reflection).
pub fn world_layers() -> RenderLayers {
    RenderLayers::from_layers(&[0, REFLECT_LAYER])
}

/// Where the eye is relative to the map's water this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WaterView {
    /// The water material whose volume holds the eye (index into the map's
    /// water materials).
    pub under: Option<usize>,
    /// Its fog colour (gamma), what an under-water view clears to.
    pub clear: Option<[f32; 3]>,
    /// Under water that refracts (a Water material with refraction on,
    /// water drawn): the original renders the refraction view (the
    /// above-water world and the 2D and 3D sky) into the back buffer first
    /// and the main view, which draws no sky, clears only depth over it
    /// (spec water.md section 9). So the sky shows through the surface
    /// from below and wherever the view draws nothing. Without refraction
    /// (fog-only water, `r_WaterDrawRefraction 0`) the view clears to the
    /// fog colour and no sky is drawn.
    pub sky_through_surface: bool,
    /// Its surface height (meters).
    pub plane: Option<f32>,
    /// With the eye above water, the surface the near plane crosses (spec
    /// section 3, "Intersection view"): its material and height. What lies
    /// below it is drawn with the water's height fog.
    pub intersect: Option<(usize, f32)>,
}

/// Water fog modes, as shaders take them (`render::fog::FogUniform`'s
/// `water_color.w`): off, range fog under water, height fog at the surface.
pub const WATER_FOG_OFF: f32 = 0.0;
pub const WATER_FOG_RANGE: f32 = 1.0;
pub const WATER_FOG_HEIGHT: f32 = 2.0;

impl WaterView {
    /// The water fog that views use now: (material, surface height, mode
    /// `WATER_FOG_RANGE` or `WATER_FOG_HEIGHT`).
    pub fn fog(&self) -> Option<(usize, f32, f32)> {
        match (self.under, self.plane, self.intersect) {
            (Some(i), Some(plane), _) => Some((i, plane, WATER_FOG_RANGE)),
            (None, _, Some((i, plane))) => Some((i, plane, WATER_FOG_HEIGHT)),
            _ => None,
        }
    }
}

/// On the reflection camera: the clusters around its water surface (the
/// reflection shows what is potentially visible from them; `render::vis::cull`).
#[derive(Component, Clone, PartialEq)]
pub struct ReflectionClusters(pub Vec<u32>);

/// The water's height fog on a point below the surface (spec section 8):
/// `fog_plane` is the surface plus the 2-unit fudge, `depth` the point's
/// view depth, `k` one over the fog range. Same units throughout.
pub fn height_fog(eye_height: f32, point_height: f32, fog_plane: f32, depth: f32, k: f32) -> f32 {
    let below = fog_plane - point_height;
    let path = eye_height - point_height;
    let h = if path > 0.0 {
        (below / path).clamp(0.0, 1.0)
    } else {
        1.0
    };
    (h * depth * k).clamp(0.0, 1.0)
}
