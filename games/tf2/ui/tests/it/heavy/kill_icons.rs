//! The kill feed's icons from the user's install (passes with a note
//! without it).

use bevy::prelude::*;

use super::install::install_mount;

/// The kill feed's icons (hud.md 3.6): each M2 gun's cell cut from the
/// install's `hud/d_images` (the Rocket Launcher's the rocket's), 96 x 32.
#[test]
fn the_guns_kill_icons_are_cut_from_the_sheet() {
    use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
    if install_mount().is_none() {
        return;
    }
    let mut app = App::new();
    app.init_resource::<Assets<Image>>().init_resource::<WeaponIcons>().add_systems(Update, tf2_ui::systems::register_kill_icons);
    app.update();
    let icons = app.world().resource::<WeaponIcons>();
    use tf2::game::weapons::{MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN};
    for gun in [SCATTERGUN, ROCKET_LAUNCHER, SHOTGUN, MINIGUN] {
        match icons.icon(gun) {
            Some(WeaponIcon::Image { image, size }) => {
                assert_eq!(size, Vec2::new(96.0, 32.0), "{gun}");
                let img = app.world().resource::<Assets<Image>>().get(&image).unwrap();
                let opaque = img.data.as_ref().unwrap().chunks(4).filter(|p| p[3] > 128).count();
                assert!(opaque > 50, "{gun}: an empty cell ({opaque} opaque pixels)");
            }
            other => panic!("{gun}: {other:?}"),
        }
    }
}
