//! Materials and meshes built from map meshes: the standard and prop
//! materials, and the fog parameters map shaders take.

use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};

use super::prop_material::{PropMaterial, PropParams};
#[allow(unused_imports)]
use crate::map::*;

/// Bevy adds `lightmap * lightmap_exposure` as light, then applies the
/// camera's exposure. This cancels the default camera exposure (EV100 9.7)
/// so a lightmap value of 1.0 shows a texture at its own brightness.
pub const LIGHTMAP_EXPOSURE: f32 = 1.2 * 831.746_4; // 1.2 * 2^9.7

pub fn build_mesh(m: &MapMesh, with_lightmap: bool) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs.clone());
    if with_lightmap {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, m.lightmap_uvs.clone());
    }
    mesh.insert_indices(Indices::U32(m.indices.clone()));
    mesh
}

pub fn build_material(
    m: &MapMesh,
    textures: &[Handle<Image>],
    view: MapDebugView,
    light_scale: f32,
) -> StandardMaterial {
    let [r, g, b] = m.color;
    let lighting_only = matches!(view, MapDebugView::Lighting { .. });
    let scale = if let MapDebugView::Lighting { scale } = view {
        scale
    } else {
        1.0
    };
    let tint = m.tint.map_or(LinearRgba::WHITE, |[r, g, b]| LinearRgba::rgb(r, g, b));
    StandardMaterial {
        base_color: if m.texture.is_some() || lighting_only {
            tint.into()
        } else {
            {
                let c = Color::srgb_u8(r, g, b).to_linear();
                LinearRgba::rgb(c.red * tint.red, c.green * tint.green, c.blue * tint.blue).into()
            }
        },
        base_color_texture: m.texture.filter(|_| !lighting_only).map(|i| textures[i].clone()),
        unlit: view == MapDebugView::Albedo,
        perceptual_roughness: 0.95,
        reflectance: 0.2,
        alpha_mode: match m.alpha {
            MapAlpha::Opaque => AlphaMode::Opaque,
            MapAlpha::Mask(cutoff) => AlphaMode::Mask(cutoff),
            MapAlpha::Blend => AlphaMode::Blend,
            MapAlpha::Add => AlphaMode::Add,
        },
        double_sided: m.double_sided,
        cull_mode: if m.double_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        lightmap_exposure: LIGHTMAP_EXPOSURE * scale * light_scale,
        ..default()
    }
}

/// The material of a prop mesh lit by a light probe (`PropMaterial`),
/// without its cubemap; `skybox`: in the 3D skybox (the sky_camera's fog).
pub fn lit_prop_material(
    m: &MapMesh,
    textures: &[Handle<Image>],
    data: &MapData,
    view: MapDebugView,
    skybox: bool,
) -> PropMaterial {
    let [r, g, b] = m.color;
    let lighting_only = matches!(view, MapDebugView::Lighting { .. });
    let tint = m.tint.map_or(Vec4::ONE, |[r, g, b]| Vec4::new(r, g, b, 1.0));
    let mut material = PropMaterial {
        params: PropParams {
            base_color: if m.texture.is_some() || lighting_only {
                tint
            } else {
                Color::srgb_u8(r, g, b).to_linear().to_vec4() * tint
            },
            alpha_cutoff: if let MapAlpha::Mask(c) = m.alpha { c } else { 0.0 },
            fog_color: fog_color(material_fog(data, view, skybox).as_ref()),
            fog_range: fog_range(material_fog(data, view, skybox).as_ref()),
            translucent: m.alpha.shader_mode(),
            dynamic: if m.unlit || view != MapDebugView::Normal {
                0.0
            } else {
                1.0
            },
            base_uv_u: m.base_transform.shader_rows()[0],
            base_uv_v: m.base_transform.shader_rows()[1],
            detail: m
                .detail
                .filter(|_| !lighting_only)
                .map_or(0.0, |d| d.mode.shader_value()),
            detail_factor: m.detail.map_or(0.0, |d| d.factor),
            detail_scale: m.detail.map_or(Vec2::ONE, |d| Vec2::from_array(d.scale)),
            detail_tint: m.detail.map_or(Vec4::ONE, |d| Vec3::from_array(d.tint).extend(1.0)),
            selfillum: match m.selfillum {
                Some(MapSelfIllum { mask: Some(_), .. }) => 2.0,
                Some(_) => 1.0,
                None => 0.0,
            },
            selfillum_tint: m.selfillum.map_or(Vec4::ONE, |s| Vec3::from_array(s.tint).extend(1.0)),
            ..default()
        },
        base: m.texture.filter(|_| !lighting_only).map(|i| textures[i].clone()),
        envmap: None,
        envmap_mask: None,
        detail: m.detail.filter(|_| !lighting_only).map(|d| textures[d.texture].clone()),
        selfillum_mask: m.selfillum.and_then(|s| s.mask).map(|i| textures[i].clone()),
        alpha_mode: m.alpha.shader_alpha_mode(),
        double_sided: m.double_sided,
        cull_front: false,
        layer: None,
        skin_frames: m
            .skin_frames
            .as_ref()
            .filter(|_| !lighting_only)
            .and_then(|s| s.frames.iter().map(|&f| textures.get(f).cloned()).collect::<Option<Vec<_>>>())
            .unwrap_or_default(),
        skin_frame_ms: m.skin_frames.as_ref().map_or(0, |s| s.frame_ms),
    };
    if let Some(layer) = m.layer.as_ref().filter(|_| !lighting_only) {
        set_prop_layer(&mut material, layer, textures);
    }
    material
}

/// Make `material` draw `layer` (`map::MapLayer`) over its base; one
/// whose texture didn't resolve draws nothing more.
pub fn set_prop_layer(material: &mut PropMaterial, layer: &MapLayer, textures: &[Handle<Image>]) {
    let Some(image) = layer.texture.and_then(|t| textures.get(t)) else { return };
    material.layer = Some(image.clone());
    let p = &mut material.params;
    p.layer = 1.0
        + match layer.combine {
            LayerCombine::MaskedAdd => 0.0,
            LayerCombine::Add => 1.0,
            LayerCombine::AlphaLerp => 2.0,
            LayerCombine::Gloss => 3.0,
        };
    p.layer_coords = match layer.coords {
        LayerCoords::CameraReflection => 0.0,
        LayerCoords::CameraNormal => 1.0,
        LayerCoords::SphereMap => 2.0,
        LayerCoords::WorldReflection => 3.0,
    };
    p.layer_address = match layer.address {
        LayerAddress::Wrap => 0.0,
        LayerAddress::Clamp => 1.0,
        LayerAddress::Mirror => 2.0,
    };
    p.layer_factor = layer.factor;
    p.layer_light = layer.light;
}

/// Make `material` reflect `cube` as `env` says (Source `$envmap`).
/// `normal_map`: the mesh's `$bumpmap`, whose alpha is the mask under
/// `$normalmapalphaenvmapmask` (specs/cs_source/shaders.md 2 and 4).
pub fn set_prop_envmap(
    material: &mut PropMaterial,
    env: &MapEnvmap,
    cube: Handle<Image>,
    textures: &[Handle<Image>],
    normal_map: Option<usize>,
) {
    material.envmap = Some(cube);
    material.params.envmap = 1.0;
    let (mode, mask) = prop_envmap_mask(env.mask, normal_map);
    material.params.envmap_mask = mode;
    material.envmap_mask = mask.and_then(|t| textures.get(t).cloned());
    material.params.envmap_contrast = env.contrast;
    material.params.envmap_saturation = env.saturation;
    material.params.envmap_tint = Vec3::from_array(env.tint.map(parameter_gamma_to_linear)).extend(1.0);
}

/// The prop shader's mask mode for `mask` (`PropParams::envmap_mask`) and
/// the texture it samples: the normal map's alpha (1), one minus the base
/// alpha (2), a mask texture's colour (3), or none (0). A normal-alpha mask
/// without a normal map masks nothing (the loader only gives it to bumped
/// materials).
pub fn prop_envmap_mask(mask: EnvmapMask, normal_map: Option<usize>) -> (f32, Option<usize>) {
    match (mask, normal_map) {
        (EnvmapMask::NormalAlpha, Some(n)) => (1.0, Some(n)),
        (EnvmapMask::BaseAlphaInverted, _) => (2.0, None),
        (EnvmapMask::Texture(t), _) => (3.0, Some(t)),
        _ => (0.0, None),
    }
}

/// Fog color for shaders: linear RGB, w = 1 when fog is on.
pub fn fog_color(fog: Option<&MapFog>) -> Vec4 {
    fog.map_or(Vec4::ZERO, |f| {
        let c = Color::srgb(f.color[0], f.color[1], f.color[2]).to_linear();
        Vec4::new(c.red, c.green, c.blue, 1.0)
    })
}

/// The fog a map material gets: the world's, or in the 3D skybox the
/// sky_camera's own (specs/cs_source/shadows_sky.md: the skybox pass draws
/// with it), its distances from world scale to the skybox's (divided by
/// the scale: the sky camera sees the skybox from `eye / scale`). None in
/// debug views.
pub fn material_fog(data: &MapData, view: MapDebugView, skybox: bool) -> Option<MapFog> {
    if view != MapDebugView::Normal {
        return None;
    }
    if !skybox {
        return data.fog.clone();
    }
    let cam = data.sky_camera.as_ref()?;
    cam.fog.as_ref().map(|f| MapFog {
        start: f.start / cam.scale,
        end: f.end / cam.scale,
        ..f.clone()
    })
}

/// Fog start, end (meters) and max density for shaders; w 1 for linear
/// fog (`MapFog::linear`), 0 for Source's squared.
pub fn fog_range(fog: Option<&MapFog>) -> Vec4 {
    fog.map_or(Vec4::new(0.0, 1.0, 0.0, 0.0), |f| {
        Vec4::new(f.start, f.end.max(f.start + 0.01), f.max_density, if f.linear { 1.0 } else { 0.0 })
    })
}

#[cfg(test)]
mod fog_tests {
    use super::*;

    fn fog(start: f32, end: f32) -> MapFog {
        MapFog {
            color: [0.5, 0.6, 0.7],
            start,
            end,
            max_density: 0.8,
            linear: false,
        }
    }

    /// The world's materials take the world's fog; the 3D skybox's take
    /// the sky_camera's, its distances in skybox space (divided by the
    /// scale); debug views none.
    #[test]
    fn skybox_materials_take_the_sky_cameras_fog() {
        let data = MapData {
            fog: Some(fog(10.0, 100.0)),
            sky_camera: Some(MapSkyCamera {
                origin: Vec3::ZERO,
                scale: 4.0,
                fog: Some(fog(12.0, 380.0)),
            }),
            ..default()
        };
        let world = material_fog(&data, MapDebugView::Normal, false).unwrap();
        assert_eq!((world.start, world.end), (10.0, 100.0));
        let sky = material_fog(&data, MapDebugView::Normal, true).unwrap();
        assert_eq!((sky.start, sky.end, sky.max_density), (3.0, 95.0, 0.8));
        assert_eq!(fog_color(Some(&sky)).w, 1.0, "on");
        assert!(material_fog(&data, MapDebugView::Albedo, true).is_none());
        // No sky_camera fog: the skybox is unfogged, whatever the world's.
        let unfogged = MapData {
            sky_camera: Some(MapSkyCamera {
                origin: Vec3::ZERO,
                scale: 16.0,
                fog: None,
            }),
            ..data
        };
        assert!(material_fog(&unfogged, MapDebugView::Normal, true).is_none());
        assert!(material_fog(&unfogged, MapDebugView::Normal, false).is_some());
    }

    /// Linear fog (LithTech's) goes toward the colour by f, Source's by f².
    #[test]
    fn linear_fog_is_not_squared() {
        let mut f = fog(0.0, 100.0);
        f.max_density = 1.0;
        let at = |f: &MapFog| super::super::fog::fog_at(&super::super::fog::world_fog(Some(f)), Vec3::ZERO, Vec3::ZERO, 50.0).1;
        assert!((at(&f) - 0.25).abs() < 1e-6);
        f.linear = true;
        assert_eq!(fog_range(Some(&f)).w, 1.0);
        assert!((at(&f) - 0.5).abs() < 1e-6);
    }
}

#[cfg(test)]
mod envmap_mask_tests {
    use super::*;

    /// `$normalmapalphaenvmapmask` on a model masks by the normal map's
    /// alpha (specs/cs_source/shaders.md 2, 4): cs_militia's ammo can and
    /// cable spool were mirrors when the mask was dropped.
    #[test]
    fn model_reflections_keep_their_masks() {
        assert_eq!(prop_envmap_mask(EnvmapMask::NormalAlpha, Some(7)), (1.0, Some(7)));
        assert_eq!(prop_envmap_mask(EnvmapMask::NormalAlpha, None), (0.0, None));
        assert_eq!(prop_envmap_mask(EnvmapMask::BaseAlphaInverted, Some(7)), (2.0, None));
        assert_eq!(prop_envmap_mask(EnvmapMask::Texture(3), Some(7)), (3.0, Some(3)));
        assert_eq!(prop_envmap_mask(EnvmapMask::None, Some(7)), (0.0, None));
    }
}
