//! A game's kill icon fitted by its visible ink (`chimera_ui::icon_ink`),
//! for the Signal HUD (the kill feed, the race chip, the board, the
//! rank-up toast: `signal::gun_icon`, the feed's headshot mark): each
//! game's art carries its own padding (Combat Arms' images are painted in
//! their texture's top rows, an icon font's glyph sits on its baseline in
//! its em), so a box sized from the art's whole extent shows the guns
//! small and at different heights. Here the ink alone is fitted into the
//! box (`h` tall, at most `w_max` wide; its width follows the ink's
//! shape), centred, so every game's guns read at one size beside the
//! names (playtest 7: "the icons are too small").
//!
//! `spawn` makes the node (`InkIcon`); `fit_ink_icons` gives it its size
//! and image once the art has loaded: an image drawn by its lit ink's
//! rectangle (`IconInk::image`), a glyph rasterized from its outline at
//! the node's physical height (`IconInk::glyph_image`, tinted). Art that
//! has no ink to find (an image whose pixels aren't kept) is drawn whole.
//!
//! Known shortcuts: the node is empty (0 wide) until its art has loaded:
//! a row built before then gains its icon a frame later.

use bevy::prelude::*;
use chimera_ui::icon_ink::{IconInk, Ink};

use super::weapon_icons::WeaponIcon;

/// An icon waiting to be fitted (module doc).
#[derive(Component, Clone, Debug)]
pub struct InkIcon {
    pub icon: WeaponIcon,
    /// The box: `h` logical px tall, at most `w_max` wide.
    pub h: f32,
    pub w_max: f32,
    /// The UI scale factor (physical px per logical px).
    pub scale: f32,
    /// A glyph's tint (an image keeps its colours).
    pub color: Color,
}

/// An `InkIcon` that has been fitted (or drawn as text).
#[derive(Component)]
pub struct InkFitted;

/// Spawns `icon` in a box `h` tall and at most `w_max` wide (logical
/// px), fitted by its ink; its node.
pub fn spawn(p: &mut ChildSpawnerCommands, icon: &WeaponIcon, h: f32, w_max: f32, scale: f32, color: Color) -> Entity {
    p.spawn((
        Node { width: px(0.0), height: px(h), flex_shrink: 0.0, ..default() },
        InkIcon { icon: icon.clone(), h, w_max, scale: scale.max(1e-3), color },
        Pickable::IGNORE,
    ))
    .id()
}

/// The logical size `ink` (any units) takes fitted into `h` by `w_max`.
pub fn fitted(ink: Vec2, h: f32, w_max: f32) -> Vec2 {
    let ink = ink.max(Vec2::splat(1e-6));
    let k = (h / ink.y).min(w_max / ink.x);
    ink * k
}

/// Gives each waiting `InkIcon` its size and image (module doc).
pub fn fit_ink_icons(
    mut ink: ResMut<IconInk>,
    mut images: Option<ResMut<Assets<Image>>>,
    fonts: Option<Res<Assets<Font>>>,
    mut waiting: Query<(Entity, &InkIcon, &mut Node), Without<InkFitted>>,
    mut commands: Commands,
) {
    let Some(images) = images.as_deref_mut() else { return };
    for (e, icon, mut node) in &mut waiting {
        match &icon.icon {
            WeaponIcon::Image { image, size } => {
                let rect = match ink.image(images, image) {
                    Ink::Pending => continue,
                    Ink::Found(r) => r,
                    Ink::None => Rect::from_corners(Vec2::ZERO, *size),
                };
                let s = fitted(rect.size(), icon.h, icon.w_max);
                node.width = px(s.x);
                node.height = px(s.y);
                commands.entity(e).insert((InkFitted, ImageNode { rect: Some(rect), ..ImageNode::new(image.clone()) }));
            }
            WeaponIcon::Glyph { font, ch, em } => {
                let Some(fonts) = fonts.as_deref() else { continue };
                let shape = match ink.glyph(fonts, font, *ch) {
                    Ink::Pending => continue,
                    Ink::Found(g) => g,
                    Ink::None => {
                        // No outline to fit: the glyph as text, as before.
                        node.width = Val::Auto;
                        commands.entity(e).insert(InkFitted).with_children(|c| {
                            super::weapon_icons::spawn_weapon_icon(c, &icon.icon, icon.h / em.max(1e-3), icon.h, None);
                        });
                        continue;
                    }
                };
                let s = fitted(shape.ink.size(), icon.h, icon.w_max);
                let phys = (s.y * icon.scale).round().max(1.0) as u32;
                let Ink::Found((_, image)) = ink.glyph_image(fonts, images, font, *ch, phys) else { continue };
                node.width = px(s.x);
                node.height = px(s.y);
                commands.entity(e).insert((InkFitted, ImageNode { color: icon.color, ..ImageNode::new(image) }));
            }
        }
    }
}

/// `fit_ink_icons` in `PostUpdate` before the UI's layout, once.
pub fn setup(app: &mut App) {
    #[derive(Resource)]
    struct InkIconsSetUp;
    if app.world().contains_resource::<InkIconsSetUp>() {
        return;
    }
    app.insert_resource(InkIconsSetUp)
        .init_resource::<IconInk>()
        .add_systems(PostUpdate, fit_ink_icons.before(bevy::ui::UiSystems::Prepare));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wide icon fills the height; a very wide one the width.
    #[test]
    fn ink_fits_its_box() {
        assert_eq!(fitted(Vec2::new(40.0, 10.0), 20.0, 100.0), Vec2::new(80.0, 20.0));
        assert_eq!(fitted(Vec2::new(100.0, 10.0), 20.0, 50.0), Vec2::new(50.0, 5.0));
    }
}
