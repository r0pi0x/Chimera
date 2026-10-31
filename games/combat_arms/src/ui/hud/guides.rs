//! The on-screen guide markers (specs/combat_arms/world_objects.md
//! Follow-up (2026-10-31b) 1.6, the main HUD layout's `WorldItem`): one per
//! map marker that is on and has a guide picture (`game::objects::markers::
//! CaMarkers`, the trigger resource table's), drawn over the marker's
//! place each frame at its picture's size, with the distance in whole
//! metres under it when its record says `ShowDist`.
//!
//! Known shortcuts: the line test that dims a marker to alpha 125 isn't in
//! the material, so markers are drawn fully opaque; a marker behind the
//! camera isn't drawn (the spec doesn't say where it goes).

use std::collections::HashSet;

use bevy::prelude::*;

use super::{CaHud, HUD_Z, layer, radar::ensure_picture};
use crate::{
    client::{FirstPersonCamera, fonts::UiFonts, hud::HudRoot},
    core::LocalPlayer,
    game::objects::markers::{CaMarkers, distance_text, guide_alpha},
    ui::draw::CaArt,
};

/// The guide markers' layer.
#[derive(Component)]
pub struct GuideRoot;

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((GuideRoot, layer(HUD_Z - 2), HudRoot(crate::game::GAME)));
}

/// Fallback size of a guide picture without the install (px).
const FALLBACK: f32 = 24.0;

#[allow(clippy::too_many_arguments)]
pub fn draw_guides(
    markers: Option<Res<CaMarkers>>,
    hud: Res<CaHud>,
    fonts: Option<Res<UiFonts>>,
    mut art: Option<ResMut<CaArt>>,
    mut images: Option<ResMut<Assets<Image>>>,
    root: Single<Entity, With<GuideRoot>>,
    camera: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
    me: Query<&GlobalTransform, With<LocalPlayer>>,
    mut failed: Local<HashSet<String>>,
    mut commands: Commands,
) {
    commands.entity(*root).despawn_related::<Children>();
    let Some(markers) = markers else { return };
    let Some((cam, at)) = camera.iter().next() else { return };
    let units = crate::game::movement::movement_data().meters_per_unit;
    let mine = me.iter().next().map_or(at.translation(), |t| t.translation());
    let pts = 11.0 * hud.screen.yr();
    for m in &markers.0 {
        if let (Some(g), Some(a), Some(i)) = (&m.icons.guide, art.as_mut(), images.as_mut()) {
            ensure_picture(g, a, i, &mut failed);
        }
    }
    let art = art.as_deref();
    commands.entity(*root).with_children(|p| {
        for m in &markers.0 {
            let Some(guide) = &m.icons.guide else { continue };
            if (m.at - at.translation()).dot(at.forward().as_vec3()) <= 0.0 {
                continue;
            }
            let Ok(s) = cam.world_to_viewport(at, m.at) else { continue };
            let alpha = guide_alpha(false) as f32 / 255.0;
            let picture = art.and_then(|a| a.get(guide)).cloned();
            let size = picture.as_ref().map_or(Vec2::splat(FALLBACK), |(_, d)| d.as_vec2());
            let mut node = p.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(s.x - size.x / 2.0),
                top: px(s.y - size.y / 2.0),
                width: px(size.x.max(80.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                ..default()
            });
            node.with_children(|c| {
                match picture {
                    Some((h, _)) => {
                        c.spawn((
                            Node {
                                width: px(size.x),
                                height: px(size.y),
                                ..default()
                            },
                            ImageNode::new(h).with_color(Color::srgba(1.0, 1.0, 1.0, alpha)),
                        ));
                    }
                    None => {
                        c.spawn((
                            Node {
                                width: px(size.x),
                                height: px(size.y),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 0.8, 0.2, 0.6 * alpha)),
                        ));
                    }
                }
                if m.icons.show_dist {
                    let d = (m.at - mine).length() / units.max(1e-6);
                    let font = fonts.as_deref().map_or_else(
                        || TextFont {
                            font_size: FontSize::Px(pts),
                            ..default()
                        },
                        |f| f.em("Tahoma", true, pts),
                    );
                    c.spawn((Text::new(distance_text(d)), font, TextColor(Color::srgba(1.0, 1.0, 1.0, alpha))));
                }
            });
        }
    });
}
