//! Debug overlays drawn with gizmos: the nav mesh around you
//! (`chimera_drawnav 1`: areas coloured by place, links between them,
//! ladders and their get-on points) and ragdolls (`chimera_ragdoll_debug
//! 1`: each body's bounds and axes, each joint from its parent body's
//! anchor to the child body). A game's own overlays (CS:S's bots:
//! `games::cs_source::ui::debug`) come from its front end. `fog_enable` (the
//! map's fog on or off in the materials: `render::fog::FogSetting`) is
//! registered here too, beside the other debug switches.

use bevy::prelude::*;

use crate::{
    console::resource_cvar,
    core::LocalPlayer,
    map::{
        nav::NavMesh,
        ragdoll::{RagdollBody, RagdollJoint, RagdollSettings},
    },
};

pub struct DebugViewsPlugin;

impl Plugin for DebugViewsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugViews>()
            .add_systems(Update, (draw_nav, draw_ragdolls));
        resource_cvar::<DebugViews, u8>(
            app,
            "chimera:ragdoll_debug",
            "1: draw ragdoll bodies (bounds, axes) and joints (parent anchor to child).",
            |d| &mut d.ragdolls,
        );
        resource_cvar::<RagdollSettings, u8>(
            app,
            "cl_ragdoll_physics_enable",
            "Enable/disable ragdoll physics (0: the dead just vanish).",
            |s| &mut s.enabled,
        );
        resource_cvar::<RagdollSettings, f32>(
            app,
            "ragdoll_sleepaftertime",
            "After this many seconds of being basically stationary, the ragdoll will go to sleep.",
            |s| &mut s.sleep_after,
        );
        resource_cvar::<crate::render::fog::FogSetting, u8>(
            app,
            "fog_enable",
            "0: the map's fog is off (world, props, ropes, decals, particles; water fog stays); 1: on (default).",
            |f| &mut f.enabled,
        );
        resource_cvar::<DebugViews, u8>(
            app,
            "chimera:drawnav",
            "1: outline nav areas near you (coloured by place) and their links; 2: all of them.",
            |d| &mut d.nav,
        );
    }
}

#[derive(Resource, Default)]
struct DebugViews {
    nav: u8,
    ragdolls: u8,
}

/// Areas within this of you are drawn at `chimera_drawnav 1`, m.
const NAV_RADIUS: f32 = 25.0;
/// Lift off the floor, m.
const LIFT: f32 = 0.05;

fn place_color(place: Option<usize>) -> Color {
    match place {
        // A stable hue per place.
        Some(p) => Color::hsl((p as f32 * 67.0) % 360.0, 0.8, 0.55),
        None => Color::srgb(0.6, 0.6, 0.6),
    }
}

fn draw_nav(
    views: Res<DebugViews>,
    nav: Option<Res<NavMesh>>,
    me: Option<Single<&GlobalTransform, With<LocalPlayer>>>,
    mut gizmos: Gizmos,
) {
    let (Some(nav), true) = (nav, views.nav > 0) else {
        return;
    };
    let here = me.map(|t| t.translation());
    for a in &nav.areas {
        if views.nav == 1 && here.is_some_and(|h| h.distance(a.center) > NAV_RADIUS) {
            continue;
        }
        let [h0, h1, h2, h3] = a.heights;
        let corners = [
            Vec3::new(a.min.x, h0 + LIFT, a.min.y),
            Vec3::new(a.max.x, h1 + LIFT, a.min.y),
            Vec3::new(a.max.x, h2 + LIFT, a.max.y),
            Vec3::new(a.min.x, h3 + LIFT, a.max.y),
        ];
        let color = place_color(a.place);
        gizmos.linestrip(corners.iter().copied().chain(std::iter::once(corners[0])), color);
        let from = a.center.with_y(a.height_at(a.center.x, a.center.z) + LIFT);
        for (to, _) in &a.links {
            let b = &nav.areas[*to];
            let end = b.center.with_y(b.height_at(b.center.x, b.center.z) + LIFT);
            // Half the way: two-way links meet in the middle.
            gizmos.line(from, from.lerp(end, 0.5), color.with_alpha(0.5));
        }
    }
    // Ladders: the climbable line (yellow), the point in front of the foot
    // bots get on from (green) and the one behind the top they go down
    // from (orange), 32 units out (nav spec, path building).
    let out = 32.0 * 0.0254;
    for l in &nav.ladders {
        if views.nav == 1 && here.is_some_and(|h| h.distance(l.bottom) > NAV_RADIUS) {
            continue;
        }
        gizmos.line(l.bottom, l.top, Color::srgb(1.0, 0.9, 0.2));
        let cross = |gizmos: &mut Gizmos, p: Vec3, c: Color| {
            gizmos.line(p - Vec3::X * 0.15, p + Vec3::X * 0.15, c);
            gizmos.line(p - Vec3::Z * 0.15, p + Vec3::Z * 0.15, c);
            gizmos.line(p, p + Vec3::Y * 0.3, c);
        };
        cross(&mut gizmos, l.bottom + l.normal * out, Color::srgb(0.3, 1.0, 0.3));
        cross(&mut gizmos, l.top - l.normal * out, Color::srgb(1.0, 0.5, 0.1));
    }
}

fn draw_ragdolls(
    views: Res<DebugViews>,
    bodies: Query<(&GlobalTransform, &avian3d::prelude::ColliderAabb), With<RagdollBody>>,
    joints: Query<&RagdollJoint>,
    mut gizmos: Gizmos,
) {
    if views.ragdolls == 0 {
        return;
    }
    for (t, aabb) in &bodies {
        let centre = (aabb.min + aabb.max) / 2.0;
        gizmos.cube(
            Transform::from_translation(centre).with_scale(aabb.max - aabb.min),
            Color::srgb(0.3, 0.9, 0.4),
        );
        gizmos.axes(*t, 0.1);
    }
    for j in &joints {
        let (Ok((p, _)), Ok((c, _))) = (bodies.get(j.parent), bodies.get(j.child)) else {
            continue;
        };
        let anchor = p.transform_point(j.anchor);
        gizmos.line(p.translation(), anchor, Color::srgb(1.0, 0.9, 0.2));
        // Red when the child drifted from its anchor.
        let drift = anchor.distance(c.translation());
        let color = if drift > 0.02 {
            Color::srgb(1.0, 0.1, 0.1)
        } else {
            Color::WHITE
        };
        gizmos.sphere(Isometry3d::from_translation(anchor), 0.015, color);
    }
}
