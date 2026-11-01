//! The map logic's overlays, drawn with gizmos: `chimera_drawsound 1`
//! shows where the map's sounds reach (a sphere for each ambient_generic's
//! audible radius, green while it plays and grey while it is silent; a
//! cyan one for each env_soundscape and env_soundscape_proxy radius) and
//! `chimera_drawtriggers 1` outlines the brush triggers (the box around
//! each volume, coloured by what it does: trigger_hurt red, push blue,
//! teleport magenta, gravity cyan, remove orange, multiple and once
//! yellow, others grey; a disabled one is dim). Both list what is within
//! 80 m of you (`2`: everything). Both are check boxes on `menu debug`.

use bevy::prelude::*;

use crate::{
    console::resource_cvar,
    core::LocalPlayer,
    logic::{Logic, LogicWorld},
    map::entities::entity_to_engine,
};
use source_logic::logic::{ambient::FLAG_EVERYWHERE, classes::Class, triggers::Kind};

pub struct WorldDebugPlugin;

impl Plugin for WorldDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldViews>().add_systems(
            Update,
            draw_world_views.run_if(crate::gameplay::hud_is(crate::game::GAME)),
        );
        resource_cvar::<WorldViews, u8>(
            app,
            "chimera:drawsound",
            "1: sphere each map sound's (ambient_generic, env_soundscape) audible radius near you; 2: all of them.",
            |d| &mut d.sound,
        );
        resource_cvar::<WorldViews, u8>(
            app,
            "chimera:drawtriggers",
            "1: outline the map's brush triggers near you, coloured by kind; 2: all of them.",
            |d| &mut d.triggers,
        );
    }
}

#[derive(Resource, Default)]
struct WorldViews {
    sound: u8,
    triggers: u8,
}

/// Things within this of you are drawn at 1, m.
const NEAR: f32 = 80.0;

/// What an overlay draws, in engine space.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Sphere { at: Vec3, radius: f32, color: Color },
    Cuboid { min: Vec3, max: Vec3, color: Color },
}

fn trigger_color(kind: &Kind) -> Color {
    match kind {
        Kind::Hurt(_) => Color::srgb(1.0, 0.2, 0.2),
        Kind::Push { .. } => Color::srgb(0.3, 0.5, 1.0),
        Kind::Teleport { .. } => Color::srgb(1.0, 0.3, 1.0),
        Kind::Gravity(_) => Color::srgb(0.2, 0.9, 0.9),
        Kind::Remove => Color::srgb(1.0, 0.55, 0.1),
        Kind::Multiple { .. } => Color::srgb(1.0, 0.9, 0.2),
        Kind::Other => Color::srgb(0.6, 0.6, 0.6),
    }
}

/// The shapes `chimera_drawsound` (`sound`) and `chimera_drawtriggers`
/// (`triggers`) draw for a logic world; levels 1 keep those within `NEAR`
/// of `near` (when given), 2 everything.
pub fn shapes(world: &LogicWorld, scale: f32, sound: u8, triggers: u8, near: Option<Vec3>) -> Vec<Shape> {
    let mut out = Vec::new();
    let close = |at: Vec3, level: u8| level >= 2 || near.is_none_or(|h| at.distance(h) <= NEAR);
    for id in world.ids() {
        let Some(e) = world.get(id) else { continue };
        if sound > 0 {
            let radius = e.kv_f("radius");
            let color = match &e.class {
                Class::Ambient(a) if !e.has_flag(FLAG_EVERYWHERE) => {
                    Some(if a.playing {
                        Color::srgb(0.3, 1.0, 0.4)
                    } else {
                        Color::srgb(0.5, 0.5, 0.5)
                    })
                }
                _ if e.classname == "env_soundscape" || e.classname == "env_soundscape_proxy" => {
                    Some(Color::srgb(0.2, 0.9, 0.9))
                }
                _ => None,
            };
            if let (Some(color), true) = (color, radius > 0.0) {
                let at = entity_to_engine(e.origin, scale);
                if close(at, sound) {
                    out.push(Shape::Sphere { at, radius: radius * scale, color });
                }
            }
        }
        if triggers > 0
            && let Class::Trigger(t) = &e.class
        {
            let base = trigger_color(&t.kind);
            let color = if t.enabled { base } else { base.with_alpha(0.25) };
            for b in &t.brushes {
                let (a, c) = (entity_to_engine(b.min, scale), entity_to_engine(b.max, scale));
                if close((a + c) / 2.0, triggers) {
                    out.push(Shape::Cuboid { min: a.min(c), max: a.max(c), color });
                }
            }
        }
    }
    out
}

fn draw_world_views(
    views: Res<WorldViews>,
    logic: Option<Res<Logic>>,
    me: Option<Single<&GlobalTransform, With<LocalPlayer>>>,
    mut gizmos: Gizmos,
) {
    if views.sound == 0 && views.triggers == 0 {
        return;
    }
    let Some(logic) = logic else { return };
    let near = me.map(|t| t.translation());
    for shape in shapes(&logic.world, logic.scale, views.sound, views.triggers, near) {
        match shape {
            Shape::Sphere { at, radius, color } => {
                gizmos.sphere(Isometry3d::from_translation(at), radius, color);
            }
            Shape::Cuboid { min, max, color } => {
                gizmos.cube(
                    Transform::from_translation((min + max) / 2.0).with_scale(max - min),
                    color,
                );
            }
        }
    }
}
