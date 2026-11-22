//! The scopes' blur (specs/halo_ce/hud.md 9, S13) without the disc: the
//! camera's vertical field of view at each zoom (`draw::scope_fov`), the
//! convolution's radius by it (`draw::scope_radius`: 10.81 for the
//! pistol's out bounds at 2x, 7.92 for the others' at 2x, 11 at 8x), the
//! engine's radial blur Halo asks for (`nodes::radial_of`: two passes, the
//! mask one texel per 480-line pixel centred, r over 480), and the systems
//! that hand it to the full-screen passes (`nodes::scope_post`, cleared
//! when another game's HUD is up: `nodes::clear_blur`). The disc's
//! weapons are `heavy::scope_blur`.

use bevy::prelude::*;
use blam_formats::cache::Bounds;
use chimera_present::render::screen_post::ScreenPost;
use halo_ce_ui::hud::{
    data::Picture,
    draw::{ScopeBlur, scope_fov, scope_radius},
    nodes::{self, HaloHud, HudImages},
};

const IN: Bounds<f32> = Bounds { lower: 1.1234, upper: 0.1146 };

fn out(upper: f32) -> Bounds<f32> {
    Bounds { lower: 0.0, upper }
}

#[test]
fn the_zoomed_field_of_view_and_the_radius() {
    let nominal = 70f32.to_radians();
    let v2 = scope_fov(nominal, 2.0);
    let v8 = scope_fov(nominal, 8.0);
    assert!((v2 - 0.39672).abs() < 1e-4, "{v2}");
    assert!((v8 - 0.09747).abs() < 1e-4, "{v8}");
    assert!((scope_radius(IN, out(15.0), v2) - 10.81).abs() < 0.01, "the pistol at 2x");
    assert!((scope_radius(IN, out(11.0), v2) - 7.92).abs() < 0.01, "the rocket launcher and the sniper at 2x");
    assert!((scope_radius(IN, out(11.0), v8) - 11.0).abs() < 1e-6, "the sniper at 8x: t clamps to 1");
    let equal = Bounds { lower: 0.5, upper: 0.5 };
    assert_eq!(scope_radius(equal, Bounds { lower: 3.0, upper: 9.0 }, v2), 9.0, "equal in bounds: the out upper bound");
}

fn blur() -> ScopeBlur {
    ScopeBlur { mask: Picture { page: 7, image: 0, uv: [0.0, 0.0, 1.0, 1.0], size: [512.0, 256.0] }, radius: 7.92 }
}

#[test]
fn halo_asks_for_two_radial_passes_through_the_mask() {
    let r = nodes::radial_of(&blur(), Handle::default());
    assert_eq!((r.passes, r.mask_texels_per_height, r.centre), (2, 480.0, Vec2::splat(0.5)));
    assert!((r.radius - 7.92 / 480.0).abs() < 1e-7);
}

#[test]
fn the_request_follows_the_hud() {
    let mut app = App::new();
    app.init_resource::<ScreenPost>()
        .init_resource::<HaloHud>()
        .init_resource::<HudImages>()
        .add_systems(Update, nodes::scope_post);
    app.update();
    assert_eq!(app.world().resource::<ScreenPost>().radial, None, "nothing unzoomed");
    let mask = Handle::<Image>::default();
    app.world_mut().resource_mut::<HudImages>().0.insert((7, 0), mask.clone());
    app.world_mut().resource_mut::<HaloHud>().blur = Some(blur());
    app.update();
    assert_eq!(app.world().resource::<ScreenPost>().radial, Some(nodes::radial_of(&blur(), mask)));
    // Another game's HUD up: cleared.
    app.add_systems(Update, nodes::clear_blur.before(nodes::scope_post));
    app.update();
    assert_eq!(app.world().resource::<HaloHud>().blur, None);
    assert_eq!(app.world().resource::<ScreenPost>().radial, None);
}
