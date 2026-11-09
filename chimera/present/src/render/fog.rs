//! Source's fog for map shaders that draw after the world (ropes, runtime
//! decals, particles): the world's range fog and, under or at the water,
//! the water's fog (specs/cs_source/water.md sections 8 and 9). The maths
//! is `fog.wesl` (`chimera_present::render::fog::source_fog`); world and prop materials
//! carry the same four values in their own uniforms.
//!
//! `SceneFog` holds the current values: the map's world part, inserted
//! where the map's resources go as it spawns (`map::DrawnResource::Fog`),
//! `crate::render::water::underwater_fog` the water part as the eye moves.
//! Materials take it when created (`apply_scene_fog` would update them
//! when it changes; not scheduled).
//!
//! Source's fog shared by map shaders (`chimera_present::render::fog::source_fog`): world range
//! fog (in the 3D skybox the sky_camera's own, `render::material_fog`), the
//! water's range fog under water and height fog at the surface; `SceneFog` (set
//! by the map and `crate::render::water::underwater_fog`) taken by rope, decal and particle
//! materials; `apply_scene_fog` pushes it to them.

use bevy::prelude::*;

/// Fog uniforms (see fog.wesl): world fog linear colour (w = 1 when on)
/// and range (start, end meters, max density); water fog linear colour
/// (w: 0 off, 1 range fog under water, 2 height fog at the surface) and
/// range (start, end meters, max density, plane height meters).
#[derive(Clone, Copy, Debug, Default, PartialEq, bevy::render::render_resource::ShaderType)]
pub struct FogUniform {
    pub color: Vec4,
    pub range: Vec4,
    pub water_color: Vec4,
    pub water_range: Vec4,
}

/// Water fog mode in `FogUniform::water_color.w` (`map::water::WaterView::fog`).
pub use crate::map::water::{WATER_FOG_HEIGHT, WATER_FOG_OFF, WATER_FOG_RANGE};

/// The fog every view-space effect should use now.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneFog(pub FogUniform);

/// The world part of the fog, from the map's fog.
pub fn world_fog(fog: Option<&crate::map::MapFog>) -> FogUniform {
    FogUniform {
        color: super::fog_color(fog),
        range: super::fog_range(fog),
        ..default()
    }
}

/// How much fog a point gets (the shader's `source_fog`, for tests):
/// (colour, amount). `eye` and `point` in meters, `depth` the point's view
/// depth.
pub fn fog_at(f: &FogUniform, eye: Vec3, point: Vec3, depth: f32) -> (Vec3, f32) {
    let range_amount = |r: Vec4| {
        let x = ((depth - r.x) / (r.y - r.x)).min(r.z).clamp(0.0, 1.0);
        x * x
    };
    if f.water_color.w > 0.5 && point.y < f.water_range.w {
        let amount = if f.water_color.w > 1.5 {
            crate::map::water::height_fog(
                eye.y,
                point.y,
                f.water_range.w,
                depth,
                1.0 / (f.water_range.y - f.water_range.x).max(1e-4),
            )
        } else {
            range_amount(f.water_range)
        };
        return (f.water_color.truncate(), amount);
    }
    if f.color.w < 0.5 {
        return (Vec3::ZERO, 0.0);
    }
    // Linear fog (`MapFog::linear`): by f, not f^2.
    if f.range.w > 0.5 {
        return (f.color.truncate(), range_amount(f.range).sqrt());
    }
    (f.color.truncate(), range_amount(f.range))
}

#[derive(Resource)]
struct FogShaderLoaded;

/// `fog_enable`: whether the map's fog draws (the cvar, in
/// `client::debug_views`; for the debug UI). Off, the materials' fog
/// switch (`color.w`) is -1 where it was 1: the shaders read anything
/// under 0.5 as no fog, and on again -1 goes back to 1; the ranges and
/// colours are not touched, no material is rebuilt. Water fog (under
/// water) stays.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FogSetting {
    pub enabled: u8,
}

impl Default for FogSetting {
    fn default() -> Self {
        Self { enabled: 1 }
    }
}

/// The fog switch `w` of a material's fog colour under the setting:
/// None when it is right already.
pub fn gated_fog_switch(w: f32, on: bool) -> Option<f32> {
    if on {
        (w == -1.0).then_some(1.0)
    } else {
        (w > 0.5).then_some(-1.0)
    }
}

fn gate_all<A: Asset>(assets: &mut Assets<A>, on: bool, read: fn(&A) -> f32, write: fn(&mut A) -> &mut Vec4) {
    let ids: Vec<_> = assets.ids().collect();
    for id in ids {
        let Some(to) = assets.get(id).and_then(|m| gated_fog_switch(read(m), on)) else {
            continue;
        };
        if let Some(mut m) = assets.get_mut(id) {
            write(&mut m).w = to;
        }
    }
}

/// Put `FogSetting` on the world, prop, detail, rope, decal and particle
/// materials: while it is off (new materials arrive fogged) and once when
/// it changes.
#[allow(clippy::too_many_arguments)]
pub fn apply_fog_setting(
    setting: Res<FogSetting>,
    worlds: Option<ResMut<Assets<super::world_material::WorldMaterial>>>,
    props: Option<ResMut<Assets<super::prop_material::PropMaterial>>>,
    dormant: Option<ResMut<super::prop_material::DormantProps>>,
    details: Option<ResMut<Assets<super::detail::DetailMaterial>>>,
    ropes: Option<ResMut<Assets<super::rope_material::RopeMaterial>>>,
    decals: Option<ResMut<Assets<super::decal::DecalMaterial>>>,
    particles: Option<ResMut<Assets<super::particles::ParticleDrawMaterial>>>,
) {
    let on = setting.enabled != 0;
    if on && !setting.is_changed() {
        return;
    }
    if let Some(mut a) = worlds {
        gate_all(&mut a, on, |m| m.params.fog_color.w, |m| &mut m.params.fog_color);
    }
    if let Some(mut a) = props {
        gate_all(&mut a, on, |m| m.params.fog_color.w, |m| &mut m.params.fog_color);
    }
    // What the kit's models' drawn copies are made from.
    if let Some(mut d) = dormant {
        for m in d.values_mut() {
            if let Some(to) = gated_fog_switch(m.params.fog_color.w, on) {
                m.params.fog_color.w = to;
            }
        }
    }
    if let Some(mut a) = details {
        gate_all(&mut a, on, |m| m.params.fog.color.w, |m| &mut m.params.fog.color);
    }
    if let Some(mut a) = ropes {
        gate_all(&mut a, on, |m| m.params.fog.color.w, |m| &mut m.params.fog.color);
    }
    if let Some(mut a) = decals {
        gate_all(&mut a, on, |m| m.fog.color.w, |m| &mut m.fog.color);
    }
    if let Some(mut a) = particles {
        gate_all(&mut a, on, |m| m.fog.color.w, |m| &mut m.fog.color);
    }
}

/// Loads `chimera_present::render::fog` for the map's shaders. Needs rendering.
pub struct FogShaderPlugin;

impl Plugin for FogShaderPlugin {
    fn build(&self, app: &mut App) {
        // Several material plugins add this one; load the library once.
        if app.world().contains_resource::<FogShaderLoaded>() {
            return;
        }
        app.insert_resource(FogShaderLoaded)
            .init_resource::<FogSetting>()
            .add_systems(Update, apply_fog_setting);
        bevy::shader::load_shader_library!(app, "fog.wesl");
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// Put the current fog on rope, decal and particle materials when it
/// changes.
pub fn apply_scene_fog(
    fog: Option<Res<SceneFog>>,
    ropes: Option<ResMut<Assets<super::rope_material::RopeMaterial>>>,
    decals: Option<ResMut<Assets<super::decal::DecalMaterial>>>,
    particles: Option<ResMut<Assets<super::particles::ParticleDrawMaterial>>>,
) {
    let Some(fog) = fog.filter(|f| f.is_changed()) else {
        return;
    };
    let f = fog.0;
    if let Some(mut ropes) = ropes {
        let ids: Vec<_> = ropes.ids().collect();
        for id in ids {
            if ropes.get(id).is_some_and(|m| m.params.fog != f)
                && let Some(mut m) = ropes.get_mut(id)
            {
                m.params.fog = f;
            }
        }
    }
    if let Some(mut decals) = decals {
        let ids: Vec<_> = decals.ids().collect();
        for id in ids {
            if decals.get(id).is_some_and(|m| m.fog != f)
                && let Some(mut m) = decals.get_mut(id)
            {
                m.fog = f;
            }
        }
    }
    if let Some(mut particles) = particles {
        let ids: Vec<_> = particles.ids().collect();
        for id in ids {
            if particles.get(id).is_some_and(|m| m.fog != f)
                && let Some(mut m) = particles.get_mut(id)
            {
                m.fog = f;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water(mode: f32) -> FogUniform {
        FogUniform {
            color: Vec4::new(0.5, 0.5, 0.5, 1.0),
            range: Vec4::new(0.0, 10.0, 1.0, 0.0),
            water_color: Vec4::new(0.0, 0.1, 0.2, mode),
            water_range: Vec4::new(0.0, 1.0, 1.0, 0.0),
        }
    }

    #[test]
    fn world_fog_above_the_plane_water_fog_below() {
        let f = water(WATER_FOG_RANGE);
        // Above the plane: world range fog, f^2 at 5 of 10 m.
        let (c, a) = fog_at(&f, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, -5.0), 5.0);
        assert_eq!(c, Vec3::splat(0.5));
        assert!((a - 0.25).abs() < 1e-6);
        // Below: the water's, 0.5 of its 1 m range -> 0.25.
        let (c, a) = fog_at(&f, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, -1.0, -0.5), 0.5);
        assert_eq!(c, Vec3::new(0.0, 0.1, 0.2));
        assert!((a - 0.25).abs() < 1e-6);
        // Off: world fog everywhere.
        let (_, a) = fog_at(&water(WATER_FOG_OFF), Vec3::ZERO, Vec3::new(0.0, -1.0, -5.0), 5.0);
        assert!((a - 0.25).abs() < 1e-6);
    }

    #[test]
    fn height_fog_at_the_surface() {
        // Eye 0.1 m above the plane, a point 0.4 m below and 0.5 m away
        // (depth): 0.8 of the path under water, 0.5 m x K (1/m) -> 0.4,
        // not squared.
        let f = water(WATER_FOG_HEIGHT);
        let (c, a) = fog_at(&f, Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.0, -0.4, -0.5), 0.5);
        assert_eq!(c, Vec3::new(0.0, 0.1, 0.2));
        assert!((a - 0.4).abs() < 1e-5, "{a}");
    }

    #[derive(Asset, TypePath)]
    struct Fogged(Vec4);

    #[test]
    fn the_setting_switches_fog_off_and_back_on() {
        let mut a = Assets::<Fogged>::default();
        let fogged = a.add(Fogged(Vec4::new(0.5, 0.5, 0.5, 1.0)));
        let clear = a.add(Fogged(Vec4::new(0.5, 0.5, 0.5, 0.0)));
        let w = |a: &Assets<Fogged>, h: &Handle<Fogged>| a.get(h).unwrap().0;
        let gate = |a: &mut Assets<Fogged>, on| gate_all(a, on, |m| m.0.w, |m| &mut m.0);
        gate(&mut a, true);
        assert_eq!(w(&a, &fogged).w, 1.0);
        gate(&mut a, false);
        assert_eq!(w(&a, &fogged), Vec4::new(0.5, 0.5, 0.5, -1.0), "off, colour kept");
        assert_eq!(w(&a, &clear).w, 0.0, "a fogless material stays so");
        // Off means no fog in the shaders' test and in `fog_at`.
        let f = FogUniform {
            color: w(&a, &fogged),
            range: Vec4::new(0.0, 10.0, 1.0, 0.0),
            ..default()
        };
        assert_eq!(fog_at(&f, Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0), 5.0).1, 0.0);
        // One made while off arrives fogged and is switched.
        let late = a.add(Fogged(Vec4::new(0.1, 0.1, 0.1, 1.0)));
        gate(&mut a, false);
        assert_eq!(w(&a, &late).w, -1.0);
        gate(&mut a, true);
        assert_eq!((w(&a, &fogged).w, w(&a, &late).w, w(&a, &clear).w), (1.0, 1.0, 0.0), "on again");
    }
}
