//! Kill-feed weapon icons across games: the party apps put one game's guns
//! in another game's HUD, and a HUD draws only its own art. Each game
//! registers, in `WeaponIcons`, how its weapon ids (`game:name`) resolve to
//! an icon it can hand over as plain data (`WeaponIcon`: a glyph of an icon
//! font, or an image), its kill marks (the headshot's: `mark`) and the
//! icons of kills its guns make another way (`register_kill`, by the damage kind: a
//! gun's own grenade; `kill_icon`), and any HUD asks the registry for an id of another
//! game and draws what comes back with `spawn_weapon_icon`. A HUD that
//! gets none shows the gun's display name instead (`gun_name`). No game
//! crate knows another: the registry is keyed by the id's game part, and
//! the data crosses as handles. A game re-registers when its art loads
//! or changes (later calls replace); with nothing registered every lookup
//! is none, so plain single-game apps are unchanged.
//!
//! Known shortcuts (the kill feed centres icons' art in a line-tall box
//! itself, `kill_feed::icon_slot`; other users place them as before): an icon is drawn at the feed's own size (an image
//! scaled to the line's height, a glyph at its font's size), not at
//! the registering game's layout.

use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;

use crate::core::DamageKind;

/// What a game hands over for one of its guns.
#[derive(Clone, Debug, PartialEq)]
pub enum WeaponIcon {
    /// A glyph of an icon font; `em` is its font size in reference pixels
    /// (the game's 480-line screen).
    Glyph { font: Handle<Font>, ch: char, em: f32 },
    /// An image and its size in texels.
    Image { image: Handle<Image>, size: Vec2 },
}

type Resolver = Arc<dyn Fn(&str) -> Option<WeaponIcon> + Send + Sync>;
type KillResolver = Arc<dyn Fn(&str, DamageKind) -> Option<WeaponIcon> + Send + Sync>;

/// The games' icon resolvers by game name (the id's part before `:`, lower
/// case), their kill marks (`mark`) and their icons for what a gun's other
/// parts kill with (`kill_icon`).
#[derive(Resource, Default, Clone)]
pub struct WeaponIcons(
    HashMap<String, Resolver>,
    HashMap<(String, String), WeaponIcon>,
    HashMap<String, KillResolver>,
);

impl WeaponIcons {
    /// Registers (or replaces) `game`'s resolver for its weapon ids.
    pub fn register(&mut self, game: &str, resolver: impl Fn(&str) -> Option<WeaponIcon> + Send + Sync + 'static) {
        self.0.insert(game.to_ascii_lowercase(), Arc::new(resolver));
    }

    /// The icon of weapon `id` from its own game's resolver, if that game
    /// registered one and has art for the gun.
    pub fn icon(&self, id: &str) -> Option<WeaponIcon> {
        let (game, _) = id.split_once(':')?;
        self.0.get(&game.to_ascii_lowercase())?(id)
    }

    /// Registers (or replaces) `game`'s icons for kills its guns make in
    /// another way than their own (a rifle's grenade, a blast of the
    /// rifle, with its own icon): asked first by `kill_icon`, None falls back
    /// to `icon`.
    pub fn register_kill(&mut self, game: &str, resolver: impl Fn(&str, DamageKind) -> Option<WeaponIcon> + Send + Sync + 'static) {
        self.2.insert(game.to_ascii_lowercase(), Arc::new(resolver));
    }

    /// The icon of a kill by weapon `id` dealt as `kind`: its game's kill
    /// icon for that, else the gun's own (`icon`).
    pub fn kill_icon(&self, id: &str, kind: DamageKind) -> Option<WeaponIcon> {
        let (game, _) = id.split_once(':')?;
        self.2
            .get(&game.to_ascii_lowercase())
            .and_then(|f| f(id, kind))
            .or_else(|| self.icon(id))
    }

    /// Registers (or replaces) `game`'s kill mark `name` (`"headshot"`).
    pub fn register_mark(&mut self, game: &str, name: &str, icon: WeaponIcon) {
        self.1.insert((game.to_ascii_lowercase(), name.to_string()), icon);
    }

    /// `game`'s kill mark `name`, if it registered one: a feed drawing
    /// that game's gun draws the mark in the same art.
    pub fn mark(&self, game: &str, name: &str) -> Option<WeaponIcon> {
        self.1.get(&(game.to_ascii_lowercase(), name.to_string())).cloned()
    }
}

/// The rows of `image` that hold its art (first row, one past the last, in
/// texels; a pixel counts from 10 % opacity): an icon may be painted high in
/// a bigger texture, as Combat Arms' kill icons are (20 rows at the top of
/// 64 x 32). None for an image whose pixels aren't here as RGBA bytes.
pub fn ink_rows(image: &Image) -> Option<(f32, f32)> {
    use bevy::render::render_resource::TextureFormat as F;
    if !matches!(image.texture_descriptor.format, F::Rgba8Unorm | F::Rgba8UnormSrgb) {
        return None;
    }
    let (w, h) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_deref()?;
    if w == 0 || data.len() < w * h * 4 {
        return None;
    }
    let inked = |y: &usize| data[y * w * 4..(y + 1) * w * 4].chunks_exact(4).any(|p| p[3] >= 26);
    let first = (0..h).find(inked)?;
    let last = (0..h).rev().find(inked)?;
    Some((first as f32, last as f32 + 1.0))
}

/// Spawns `icon` as a child of `p`: a line `height` tall in pixels
/// (`scale` is pixels per reference pixel, for a glyph's size), absolutely
/// at `at` or, with none, in the parent's flex flow.
pub fn spawn_weapon_icon(p: &mut ChildSpawnerCommands, icon: &WeaponIcon, scale: f32, height: f32, at: Option<Vec2>) {
    let place = |mut n: Node| {
        if let Some(at) = at {
            n.position_type = PositionType::Absolute;
            n.left = px(at.x);
            n.top = px(at.y);
        }
        n
    };
    match icon {
        WeaponIcon::Glyph { font, ch, em } => {
            // Icon fonts draw their glyphs high in the line: lowered to
            // sit with the names.
            p.spawn((
                Text::new(ch.to_string()),
                TextColor(Color::WHITE),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(em * scale),
                    ..default()
                },
                place(Node {
                    margin: UiRect::top(px(em * scale * 0.5)),
                    ..default()
                }),
            ));
        }
        WeaponIcon::Image { image, size } => {
            let wide = if size.y > 0.0 { height * size.x / size.y } else { height };
            p.spawn((
                place(Node {
                    width: px(wide),
                    height: px(height),
                    ..default()
                }),
                ImageNode::new(image.clone()),
            ));
        }
    }
}
