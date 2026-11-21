//! Source's fog for map shaders that draw after the world (ropes, runtime
//! decals, particles): the world's range fog and, under or at the water,
//! the water's fog (specs/cs_source/water.md sections 8 and 9). The maths
//! is `fog.wesl` (`chimera_present::render::fog::source_fog`); world and prop materials
//! carry the same four values in their own uniforms.
//!
//! `SceneFog` holds the current values: the map's world part, inserted
//! where the map's resources go as it spawns (`map::DrawnResource::Fog`),
//! `crate::render::water::underwater_fog` the water part as the eye moves.
//! Materials take it when created; `apply_scene_fog` (scheduled by
//! `FogShaderPlugin`, so in every app that draws maps) puts it on the
//! existing ones when it changes: a new map's fog on materials made
//! before it, the water's as the eye goes under and back.
//!
//! Source's fog shared by map shaders (`chimera_present::render::fog::source_fog`): world range
//! fog (in the 3D skybox the sky_camera's own, `render::material_fog`), the
//! water's range fog under water and height fog at the surface; `SceneFog` (set
//! by the map and `crate::render::water::underwater_fog`) taken by rope, decal and particle
//! materials; `apply_scene_fog` pushes it to them. Surfaces take it with
//! fog.wesl's `fog_mix` (mirror: `fog_surface`): Source's fog blends in
//! linear light, linear fog (`MapFog::linear`: LithTech's, Halo's) the
//! gamma values, as those engines' framebuffers hold them.

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

/// A surface's colour after fog `(colour, amount)` (`fog_at`) of the
/// scene's fog `f`, as the world and prop shaders apply it (fog.wesl
/// `fog_mix`): toward the fog colour, but an additive surface toward
/// black, as particles are (it adds nothing where fogged out; toward the
/// fog colour it would add that colour over its whole shape: the
/// 2026-11-14 playtest's pale light cones in Combat Arms' fogged maps).
/// Linear fog (`MapFog::linear`: LithTech's and Halo's fixed-function
/// table fog) blends the gamma values, as those engines' framebuffers
/// hold them (specs/combat_arms/world_dat.md test case 14); Source's
/// blends in linear light. Colours linear.
pub fn fog_surface(color: Vec3, fog: (Vec3, f32), additive: bool, f: &FogUniform) -> Vec3 {
    if additive {
        return color * (1.0 - fog.1);
    }
    if f.range.w > 0.5 && fog.1 > 0.0 {
        let to_gamma = |c: Vec3| Vec3::from_array(c.to_array().map(|x| Color::linear_rgb(x, 0.0, 0.0).to_srgba().red));
        let to_linear = |c: Vec3| Vec3::from_array(c.to_array().map(|x| Color::srgb(x, 0.0, 0.0).to_linear().red));
        return to_linear(to_gamma(color).lerp(to_gamma(fog.0), fog.1));
    }
    color.lerp(fog.0, fog.1)
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
            .add_systems(Update, apply_fog_setting)
            // After the water sets this frame's (a no-op order where
            // there is no water plugin).
            .add_systems(PostUpdate, apply_scene_fog.after(super::water::underwater_fog));
        bevy::shader::load_shader_library!(app, "fog.wesl");
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// `to` as a material should take it: its fog switch kept off where the
/// setting turned it off (`FogSetting`, -1), so a change of fog doesn't
/// turn it back on.
fn scene_fog_for(current: &FogUniform, to: FogUniform) -> FogUniform {
    let mut to = to;
    if current.color.w == -1.0 && to.color.w > 0.5 {
        to.color.w = -1.0;
    }
    to
}

fn set_fog<A: Asset>(assets: &mut Assets<A>, f: FogUniform, read: fn(&A) -> &FogUniform, write: fn(&mut A) -> &mut FogUniform) {
    let ids: Vec<_> = assets.ids().collect();
    for id in ids {
        let Some(to) = assets.get(id).map(read).map(|now| (*now, scene_fog_for(now, f))).and_then(|(now, to)| (now != to).then_some(to)) else {
            continue;
        };
        if let Some(mut m) = assets.get_mut(id) {
            *write(&mut m) = to;
        }
    }
}

/// Put the current fog (`SceneFog`) on rope, decal and particle materials
/// when it changes (or arrives with a map), keeping `FogSetting`'s switch.
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
    if let Some(mut a) = ropes {
        set_fog(&mut a, f, |m| &m.params.fog, |m| &mut m.params.fog);
    }
    if let Some(mut a) = decals {
        set_fog(&mut a, f, |m| &m.fog, |m| &mut m.fog);
    }
    if let Some(mut a) = particles {
        set_fog(&mut a, f, |m| &m.fog, |m| &mut m.fog);
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
    fn additive_surfaces_fog_toward_black() {
        let fog = world_fog(Some(&crate::map::MapFog {
            color: [0.6, 0.6, 0.65],
            start: 0.0,
            end: 100.0,
            max_density: 1.0,
            linear: true,
        }));
        let cone = Vec3::new(0.0, 0.0, 0.0);
        let (c, a) = fog_at(&fog, Vec3::ZERO, Vec3::new(0.0, 0.0, -80.0), 80.0);
        assert!(a > 0.7, "far in the fog: {a}");
        // A black texel of a light cone adds nothing, near or far.
        assert_eq!(fog_surface(cone, (c, a), true, &fog), Vec3::ZERO);
        // A bright one fades out with the fog.
        let bright = fog_surface(Vec3::ONE, (c, a), true, &fog);
        assert!(bright.max_element() <= 1.0 - a + 1e-6, "{bright}");
        // An opaque surface goes toward the fog colour.
        let source = FogUniform {
            range: fog.range.with_w(0.0),
            ..fog
        };
        assert!(fog_surface(cone, (c, a), false, &source).distance(c * a) < 1e-5);
    }

    /// specs/combat_arms/world_dat.md test case 14: WAREHOUSE's fog
    /// (23, 23, 19), 0 to 10000 u; a surface of 0.6 at depth 5000 u (f =
    /// 0.5) is drawn 0.3451 (red, green), the gamma values blended (in
    /// linear light it would be 0.445).
    #[test]
    fn linear_fog_blends_gamma_values() {
        let fog = world_fog(Some(&crate::map::MapFog {
            color: [23.0 / 255.0, 23.0 / 255.0, 19.0 / 255.0],
            start: 0.0,
            end: 10000.0 * 0.0254,
            max_density: 1.0,
            linear: true,
        }));
        let depth = 5000.0 * 0.0254;
        let (c, a) = fog_at(&fog, Vec3::ZERO, Vec3::new(0.0, 0.0, -depth), depth);
        assert!((a - 0.5).abs() < 1e-5, "{a}");
        let surface = Color::srgb(0.6, 0.6, 0.6).to_linear();
        let out = fog_surface(Vec3::new(surface.red, surface.green, surface.blue), (c, a), false, &fog);
        let drawn = Color::linear_rgb(out.x, out.y, out.z).to_srgba();
        assert!((drawn.red - 0.3451).abs() < 1e-3 && (drawn.green - 0.3451).abs() < 1e-3, "{drawn:?}");
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
