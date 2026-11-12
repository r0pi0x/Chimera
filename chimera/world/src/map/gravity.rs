//! The physics world's gravity (avian's `Gravity`, what props, ragdolls
//! and gibs fall at): the map's own (`MapGravity`, what its load sets: a
//! Source map's `sv_gravity` read at load; avian's default without a
//! map) times the server setting `chimera:phys_gravity_scale`
//! (`GravityScale`, default 1). The setting is replicated
//! (`console::CvarScope`: a `phys_` name), so a network client's
//! ragdolls and props, which it simulates itself, fall as the server's
//! do (a party host's moon gravity, `party_kit::surprise`). Ours, not a
//! Source setting.
//!
//! No system: the setting and the map's load set `Gravity` when they
//! change (`set_map_gravity`), so the fixed tick's order stays as it was.

use avian3d::prelude::Gravity;
use bevy::prelude::*;
use chimera_core::console::{Console, ConsoleAppExt};

/// The scale's setting (full name).
pub const SCALE_CVAR: &str = "chimera:phys_gravity_scale";

/// The physics gravity the map set, before `GravityScale`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapGravity(pub Vec3);

impl Default for MapGravity {
    fn default() -> Self {
        Self(Gravity::default().0)
    }
}

/// What the map's physics gravity is multiplied by (`SCALE_CVAR`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GravityScale(pub f32);

impl Default for GravityScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The map's physics gravity is `g` (a map's load, its unload): `Gravity`
/// is it times the scale.
pub fn set_map_gravity(world: &mut World, g: Vec3) {
    world.insert_resource(MapGravity(g));
    apply(world);
}

fn apply(world: &mut World) {
    let base = world.get_resource::<MapGravity>().copied().unwrap_or_default().0;
    let scale = world.get_resource::<GravityScale>().copied().unwrap_or_default().0;
    let g = Gravity(base * scale);
    if world.get_resource::<Gravity>().map(|now| now.0) != Some(g.0) {
        world.insert_resource(g);
    }
}

/// The setting and its resources; in Chimera's physics group
/// (`physics_plugins`), so every app and test harness has it.
pub struct GravityScalePlugin;

impl Plugin for GravityScalePlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

fn plugin(app: &mut App) {
    app.init_resource::<MapGravity>().init_resource::<GravityScale>();
    app.console_cvar(
        SCALE_CVAR,
        "The physics world's gravity (props, ragdolls) times this (the server's; its clients fall with it).",
        "1",
        |w| w.get_resource::<GravityScale>().map(|s| format!("{}", s.0)),
        |w, v| {
            let s: f32 = v.trim().parse().map_err(|_| format!("not a number: {v}"))?;
            if !s.is_finite() {
                return Err(format!("not a scale: {v}"));
            }
            w.insert_resource(GravityScale(s));
            apply(w);
            Ok(())
        },
    );
    app.world_mut().resource_mut::<Console>().set_range(SCALE_CVAR, 0.0, 10.0);
}
