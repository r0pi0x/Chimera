//! The party menu's own backdrop, drawn (feature `present`, with a
//! renderer): the main menu's `MenuBackdrop` node (`main_menu`,
//! "Background") as a UI material, `menu_backdrop.wesl`, an original
//! procedural scene in code (no game's files, no texture): a slate
//! gradient in the menu's colours, warm and cool glows, three layers of
//! diagonal bands drifting at different speeds, halftone dots and a slowly
//! turning reticle. It moves on the renderer's own clock (`globals.time`),
//! so a frame changes nothing on the CPU side; the material changes only
//! while the backdrop crossfades (its opacity, `MenuBackdrop::opacity`).
//! Without this plugin (headless, the tests) the node is a flat colour.
//!
//! Known shortcuts: the renderer's clock wraps after an hour, so the
//! bands jump once an hour.

use bevy::{asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

use crate::main_menu::{MainMenuSystems, MenuBackdrop};

pub struct MenuBackdropPlugin;

impl Plugin for MenuBackdropPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "menu_backdrop.wesl");
        app.add_plugins(UiMaterialPlugin::<BackdropMaterial>::default())
            .add_systems(Update, (dress, fade).chain().after(MainMenuSystems));
    }
}

/// The backdrop's material: x the opacity.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BackdropMaterial {
    #[uniform(0)]
    pub params: Vec4,
}

impl UiMaterial for BackdropMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://party_kit/menu_backdrop.wesl".into()
    }
}

/// The menu's backdrop drawn with the material instead of its flat colour.
fn dress(
    nodes: Query<(Entity, &MenuBackdrop), Without<MaterialNode<BackdropMaterial>>>,
    mut materials: ResMut<Assets<BackdropMaterial>>,
    mut commands: Commands,
) {
    for (e, b) in &nodes {
        let m = materials.add(BackdropMaterial { params: Vec4::new(b.opacity, 0.0, 0.0, 0.0) });
        commands.entity(e).remove::<BackgroundColor>().insert(MaterialNode(m));
    }
}

/// The material's opacity follows the node's (changed only while it fades).
fn fade(
    nodes: Query<(&MenuBackdrop, &MaterialNode<BackdropMaterial>), Changed<MenuBackdrop>>,
    mut materials: ResMut<Assets<BackdropMaterial>>,
) {
    for (b, m) in &nodes {
        if let Some(mut mat) = materials.get_mut(&m.0)
            && mat.params.x != b.opacity
        {
            mat.params.x = b.opacity;
        }
    }
}
