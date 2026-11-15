//! The scope's own picture: while the local player is zoomed through a
//! sight with a `ZoomSpriteName` (`game::weapons::zoom::Scope::sprite`;
//! specs/combat_arms/weapon_classes.md 7.10), that texture is loaded from
//! the install (`effects::FxImages`, the asset server's `.dtx` loader) and
//! handed to the client shell's scope overlay (`client::hud::
//! ScopeOverlayImage`), sized as 7.10 says (`ScopeOverlayTexel`): a card
//! `ZoomSpriteScale` (default 0.001) times its texels across, 1 unit
//! ahead of the view-model camera (50 degrees vertical), so one texel is
//! `scale / tan 25 deg` of the screen's height on both axes; centred,
//! cropped by the screen, nothing filling the margins (the world shows
//! where the picture is transparent). The stock 1024 x 1024 lens is about
//! 94 % of the screen's height at every aspect; at 16:9 thin strips of the
//! world show at the sides, as in Combat Arms itself (not stretched to the
//! screen). Without a picture (a sight with none, not in the install,
//! still loading) the shell draws its placeholder.
//!
//! The focus blur (weapon_classes.md Follow-up (2026-11-14) 3): while
//! `game::weapons::CaFocusBlur` is on, its mask (`FocusTex`, from the
//! install like the picture) goes to Chimera's full-screen passes
//! (`render::screen_post::ScreenPost::focus`), which blur the finished 3D
//! frame (the scope card with it) through the mask's alpha before the
//! HUD (`focus_mask`).
//!
//! Known shortcuts: a spectated player's scope shows the shell's
//! placeholder; the stock scopes' `ZoomSpriteScale` values weren't read
//! (Open question 4), so a scope without the key uses the default; the
//! blur waits for its mask to load (no blur on the frames before).

use bevy::prelude::*;

use crate::{
    client::hud::{ScopeOverlayImage, ScopeOverlayTexel},
    core::LocalPlayer,
    game::weapons::CaZoom,
    weapon::{Inventory, Zoomed},
};

/// Puts the picture on the shell's overlay.
pub struct CaScopePlugin;

impl Plugin for CaScopePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScopeSprite>()
            .add_systems(Update, ((pick_sprite, load_sprite).chain(), focus_mask));
    }
}

/// The install path of the picture the local player's zoomed weapon wants,
/// and its `ZoomSpriteScale`.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct ScopeSprite(pub Option<(String, f32)>);

/// The view-model camera's vertical FOV, degrees (`PVModelFOV`; the
/// picture is a card in its projection).
pub const VIEW_MODEL_FOV: f32 = 50.0;

/// One texel's side as a fraction of the screen's height for a
/// `ZoomSpriteScale`: `scale / tan(FOV / 2)`.
pub fn texel_fraction(scale: f32) -> f32 {
    scale / (VIEW_MODEL_FOV.to_radians() / 2.0).tan()
}

/// The active weapon's sight picture while the owner is zoomed (a scope).
fn pick_sprite(
    player: Option<Single<(&Inventory, Option<&Zoomed>), With<LocalPlayer>>>,
    zooms: Query<&CaZoom>,
    mut sprite: ResMut<ScopeSprite>,
) {
    let want = player
        .filter(|p| p.1.is_some_and(|z| z.scope))
        .and_then(|p| p.0.active)
        .and_then(|w| zooms.get(w).ok())
        .and_then(|z| z.scope.as_ref())
        .filter(|s| !s.sprite.is_empty())
        .map(|s| (s.sprite.clone(), s.sprite_scale()));
    sprite.set_if_neq(ScopeSprite(want));
}

/// Load the picture asked for and show it once decoded.
fn load_sprite(
    sprite: Res<ScopeSprite>,
    fx: Option<ResMut<super::effects::FxImages>>,
    server: Option<Res<AssetServer>>,
    mut overlay: ResMut<ScopeOverlayImage>,
    mut texel: ResMut<ScopeOverlayTexel>,
) {
    let handle = match (&sprite.0, fx, server) {
        (Some((path, _)), Some(mut fx), Some(server)) => fx.get(path, &server).filter(|h| server.load_state(h).is_loaded()),
        _ => None,
    };
    let want = handle.as_ref().zip(sprite.0.as_ref()).map(|(_, (_, scale))| texel_fraction(*scale));
    if overlay.0 != handle {
        overlay.0 = handle;
    }
    texel.set_if_neq(ScopeOverlayTexel(want));
}

/// The focus blur's mask to the full-screen passes while it is on.
fn focus_mask(
    blur: Option<Res<crate::game::weapons::CaFocusBlur>>,
    fx: Option<ResMut<super::effects::FxImages>>,
    server: Option<Res<AssetServer>>,
    post: Option<ResMut<chimera_present::render::screen_post::ScreenPost>>,
) {
    let Some(mut post) = post else { return };
    let handle = match (blur.as_deref(), fx, server) {
        (Some(b), Some(mut fx), Some(server)) if b.on => fx.get(&b.mask, &server).filter(|h| server.load_state(h).is_loaded()),
        _ => None,
    };
    if post.focus != handle {
        post.focus = handle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::weapons::zoom::Scope;

    fn picked(sprite: &str, zoomed: bool) -> Option<(String, f32)> {
        let mut app = App::new();
        app.init_resource::<ScopeSprite>().add_systems(Update, pick_sprite);
        let mut z = CaZoom::new(default());
        z.scope = Some(Scope {
            name: "Sniper".into(),
            fov: vec![20.0],
            sprite: sprite.into(),
            sprite_scale: 0.0,
            ..Default::default()
        });
        let gun = app.world_mut().spawn(z).id();
        let mut inv = Inventory::default();
        inv.weapons.push(gun);
        inv.active = Some(gun);
        let mut me = app.world_mut().spawn((LocalPlayer, inv));
        if zoomed {
            me.insert(Zoomed {
                fov: 20.0,
                scope: true,
                sensitivity: None,
            });
        }
        app.update();
        app.world().resource::<ScopeSprite>().0.clone()
    }

    #[test]
    fn a_zoomed_sniper_selects_its_picture() {
        assert_eq!(picked("hud/scope_x.dtx", true), Some(("hud/scope_x.dtx".into(), 0.001)), "the default scale");
        assert_eq!(picked("hud/scope_x.dtx", false), None, "not zoomed");
        assert_eq!(picked("", true), None, "a sight with no picture");
    }

    /// With no install or asset server nothing is shown: the shell's
    /// placeholder stays.
    #[test]
    fn no_install_leaves_the_placeholder() {
        let mut app = App::new();
        app.init_resource::<ScopeOverlayImage>().init_resource::<ScopeOverlayTexel>();
        app.insert_resource(ScopeSprite(Some(("hud/scope_x.dtx".into(), 0.001))));
        app.add_systems(Update, load_sprite);
        app.update();
        assert!(app.world().resource::<ScopeOverlayImage>().0.is_none());
    }

    /// Cases 42-46: the card's size and place from the scale, the texture
    /// and the screen's height (`ui::scope::texel_fraction`, the shell's
    /// `scope_card`), and a given scale's own.
    #[test]
    fn the_card_scales_with_the_screens_height() {
        use crate::client::hud::scope_card;
        let card = |tex: Vec2, scale: f32, screen: Vec2| scope_card(tex, texel_fraction(scale), screen);
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.1);
        let stock = Vec2::splat(1024.0);
        // 42: 1024 x 768.
        let c = card(stock, 0.001, Vec2::new(1024.0, 768.0));
        assert!(near(c, [-331.3, -459.3, 1686.5, 1686.5]), "{c:?}");
        assert!((c[2] / 1024.0 - 1.6470).abs() < 1e-3, "px per texel");
        // The ring's outer edge (texel 173 and 856) lands at x -46.3 and 1080.2.
        let edge = |texel: f32, c: [f32; 4]| c[0] + texel * c[2] / 1024.0;
        assert!((edge(173.0, c) + 46.3).abs() < 0.2 && (edge(857.0, c) - 1080.2).abs() < 0.2);
        // 43: 1920 x 1080: strips of world beyond x 175 and 1759.
        let c = card(stock, 0.001, Vec2::new(1920.0, 1080.0));
        assert!(near(c, [-225.8, -645.8, 2371.7, 2371.7]), "{c:?}");
        assert!((edge(173.0, c) - 174.9).abs() < 0.2 && (edge(857.0, c) - 1759.0).abs() < 0.2);
        // 44: 2560 x 1440.
        let c = card(stock, 0.001, Vec2::new(2560.0, 1440.0));
        assert!(near(c, [-301.1, -861.1, 3162.2, 3162.2]), "{c:?}");
        assert!((edge(173.0, c) - 233.1).abs() < 0.2 && (edge(857.0, c) - 2345.4).abs() < 0.2);
        // 45: twice the scale, twice the size; a 512 x 256 picture at 1024 x 768.
        let c = card(stock, 0.002, Vec2::new(1920.0, 1080.0));
        assert!((c[2] - 4743.3).abs() < 0.2 && (c[3] - 4743.3).abs() < 0.2);
        let c = card(Vec2::new(512.0, 256.0), 0.001, Vec2::new(1024.0, 768.0));
        assert!((c[2] - 843.3).abs() < 0.1 && (c[3] - 421.6).abs() < 0.1, "{c:?}");
        // 46: a 256 x 256 picture is a 592.9 px square about (960, 540).
        let c = card(Vec2::splat(256.0), 0.001, Vec2::new(1920.0, 1080.0));
        assert!((c[2] - 592.9).abs() < 0.1 && (c[0] + c[2] / 2.0 - 960.0).abs() < 1e-3 && (c[1] + c[3] / 2.0 - 540.0).abs() < 1e-3);
    }
}
