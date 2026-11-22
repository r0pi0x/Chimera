//! The games' full-screen passes on the view (`render::screen_post`: the
//! scene light and the focus blur) and the view's own effects a game asks
//! for (`ViewEffects`: a camera roll, a field-of-view offset, and a turn and move in the camera's frame). The
//! passes run on the last camera drawing the view (the view model's when
//! it draws, else the world's), after colour correction and before night
//! vision and the HUD, while the game's `ScreenPost` has work; the roll
//! and the offset are added by `follow_eye` and `zoom_camera` to the
//! first-person camera.

use bevy::{core_pipeline::schedule::Core3d, prelude::*, render::RenderApp};

use super::FirstPersonCamera;
use crate::{
    map::ViewModelCamera,
    render::screen_post::{ScreenPost, ScreenPostPass, ScreenPostPlugin, ScreenPostSet},
};

/// What a game adds to the first-person view this frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewEffects {
    /// Roll about the view axis, radians.
    pub roll: f32,
    /// Added to the vertical field of view, degrees.
    pub fov: f32,
    /// A turn of the view in the camera's own frame (x right, y up, -z
    /// forward): a rotation vector (axis times radians).
    pub rotation: Vec3,
    /// A move of the camera in its own frame, metres.
    pub offset: Vec3,
}

pub struct ScreenPostClientPlugin;

impl Plugin for ScreenPostClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ScreenPostPlugin)
            .init_resource::<ViewEffects>()
            .add_systems(PostUpdate, pass_camera.after(crate::render::view_model::ViewModelCameraSet));
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        app.sub_app_mut(RenderApp).configure_sets(
            Core3d,
            ScreenPostSet
                .after(super::color_correction::ColorCorrectionSet)
                .after(super::hdr::SourceTonemapSet),
        );
    }
}

/// The passes on the last camera drawing the view while the game asks.
fn pass_camera(
    post: Res<ScreenPost>,
    first_person: Query<(Entity, &Camera, Option<&Children>), With<FirstPersonCamera>>,
    view_models: Query<&Camera, With<ViewModelCamera>>,
    marked: Query<(Entity, &ScreenPostPass)>,
    mut commands: Commands,
) {
    let target = post
        .is_on()
        .then(|| {
            first_person.iter().find(|(_, c, _)| c.is_active).map(|(e, _, children)| {
                children
                    .into_iter()
                    .flatten()
                    .find(|c| view_models.get(**c).is_ok_and(|c| c.is_active))
                    .copied()
                    .unwrap_or(e)
            })
        })
        .flatten();
    let want = ScreenPostPass::of(&post);
    for (e, pass) in &marked {
        if Some(e) != target {
            commands.entity(e).remove::<ScreenPostPass>();
        } else if *pass != want {
            commands.entity(e).insert(want.clone());
        }
    }
    if let Some(e) = target
        && !marked.contains(e)
    {
        commands.entity(e).insert(want);
    }
}
