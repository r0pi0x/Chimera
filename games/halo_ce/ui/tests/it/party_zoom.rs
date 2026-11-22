//! Halo's zoom in the party apps (`game::view::party_zoom_fov`,
//! `halo_ce_ui::party_view`): for the pistol's 2x, the sniper rifle's 2x and 8x
//! and the rocket launcher's 2x (each zoomed angle as the weapons' tick
//! sets `Zoomed::fov`), the party's zoomed view at a player's 75 and 110
//! degrees is that player's view narrowed by Halo's magnification in the
//! tangent of the half angle (and Halo's own zoomed angle at Halo's own
//! 61.52); on a client app's world camera the zoom of a Halo gun replaces
//! the shell's absolute angle and another game's gun's is untouched;
//! Halo's gun in hand keeps Halo's view-model field of view at either
//! setting; zoomed, the mouse turns at Halo's own scale (one over the
//! nominal magnification).

use bevy::prelude::*;
use chimera_client::client::{FirstPersonCamera, input::MouseSettings, video::PlayerFov};
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::{Inventory, Zoomed};
use chimera_world::map::{GameViewModelFovs, view_model::{vertical_fov, view_model_fov}};
use halo_ce::game::{
    view::{self, FOV, camera_fov, magnification, party_zoom_fov},
    weapons::{self, HaloGun, HaloWeapon, PISTOL, ROCKET_LAUNCHER, SNIPER_RIFLE},
};

/// The players' settings tested (degrees horizontal at 4:3).
const PLAYERS: [f32; 2] = [75.0, 110.0];

/// Each zoom point: (gun, level, its `Zoomed` as the weapons' tick sets it).
fn zooms() -> Vec<(&'static str, i16, Zoomed)> {
    let guns = weapons::guns();
    let mut out = Vec::new();
    for (id, levels) in [(PISTOL, 1), (SNIPER_RIFLE, 2), (ROCKET_LAUNCHER, 1)] {
        let gun = &guns.iter().find(|(g, _)| g == id).unwrap().1;
        for level in 0..levels {
            let m = gun.magnification(level);
            let z = Zoomed { fov: camera_fov((gun.camera_field_of_view / m).to_degrees()), scope: false, sensitivity: Some(1.0 / m) };
            out.push((id, level, z));
        }
    }
    out
}

fn half_tan(deg: f32) -> f32 {
    (deg.to_radians() / 2.0).tan()
}

#[test]
fn the_party_zoom_narrows_the_players_view_by_halos_magnification() {
    let z = zooms();
    let nominal: Vec<f32> = z.iter().map(|(_, _, z)| 1.0 / z.sensitivity.unwrap()).collect();
    assert_eq!(nominal, [2.0, 2.0, 8.0, 2.0], "pistol 2x, sniper 2x and 8x, rocket 2x");
    for (id, level, zoomed) in &z {
        let m = magnification(zoomed.fov);
        assert!(m > 2.0, "{id} {level}: {m}");
        // At Halo's own angle: Halo's zoomed angle exactly.
        assert!((party_zoom_fov(FOV, zoomed.fov) - zoomed.fov).abs() < 1e-3, "{id} {level}");
        for player in PLAYERS {
            let party = party_zoom_fov(player, zoomed.fov);
            assert!(party < player, "{id} {level} at {player}: {party}");
            let seen = half_tan(player) / half_tan(party);
            assert!((seen - m).abs() < 1e-3 * m, "{id} {level} at {player}: {seen} against {m}");
            // The same in the vertical the camera takes.
            let seen_v = half_tan(vertical_fov(player)) / half_tan(vertical_fov(party));
            assert!((seen_v - m).abs() < 1e-3 * m, "{id} {level} at {player} vertically");
        }
        // Wider settings zoom to wider angles.
        assert!(party_zoom_fov(110.0, zoomed.fov) > party_zoom_fov(75.0, zoomed.fov));
    }
    // The 2x zooms' magnification in the tangent: tan(30.76) / tan(15.0).
    assert!((magnification(z[0].2.fov) - 2.2208).abs() < 1e-3, "{}", magnification(z[0].2.fov));
}

/// The client's camera after the shell's zoom and Halo's party zoom.
fn camera_after(player: f32, halo_gun: bool, zoomed: Option<Zoomed>) -> f32 {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).insert_resource(PlayerFov(player));
    app.add_systems(Update, chimera_client::client::zoom_camera);
    app.init_resource::<chimera_client::client::spectate::SpecView>();
    halo_ce_ui::party_view::plugin(&mut app);
    let w = app.world_mut();
    let gun = if halo_gun {
        let (_, g) = weapons::guns().into_iter().find(|(id, _)| id == SNIPER_RIFLE).unwrap();
        w.spawn((HaloGun::new(&g), HaloWeapon(std::sync::Arc::new(g)))).id()
    } else {
        w.spawn_empty().id()
    };
    let mut inv = Inventory::default();
    inv.weapons = vec![gun];
    inv.active = Some(gun);
    let mut me = w.spawn((LocalPlayer, inv));
    if let Some(z) = zoomed {
        me.insert(z);
    }
    w.spawn((FirstPersonCamera, Projection::Perspective(PerspectiveProjection::default())));
    app.update();
    app.update();
    let w = app.world_mut();
    let p = w.query_filtered::<&Projection, With<FirstPersonCamera>>().single(w).unwrap();
    match p {
        Projection::Perspective(p) => p.fov.to_degrees(),
        _ => unreachable!(),
    }
}

#[test]
fn a_halo_guns_zoom_on_the_party_camera() {
    for (id, level, zoomed) in zooms() {
        for player in PLAYERS {
            let want = vertical_fov(party_zoom_fov(player, zoomed.fov));
            let got = camera_after(player, true, Some(zoomed));
            assert!((got - want).abs() < 1e-3, "{id} {level} at {player}: {got} against {want}");
            // Another game's scope at the same absolute angle: the shell's.
            let other = camera_after(player, false, Some(zoomed));
            assert!((other - vertical_fov(zoomed.fov)).abs() < 1e-3, "{id} {level} at {player}: {other}");
        }
    }
    // Unzoomed: the player's own.
    for player in PLAYERS {
        assert!((camera_after(player, true, None) - vertical_fov(player)).abs() < 1e-3);
    }
}

#[test]
fn the_gun_in_hand_keeps_halos_field_of_view_and_the_mouse_halos_scale() {
    for player in PLAYERS {
        let mut fovs = GameViewModelFovs(vec![view::fov()]);
        fovs.follow_player(player);
        let s = fovs.settings_for(Default::default(), Some(halo_ce::game::GAME));
        assert!((view_model_fov(&s, player) - FOV).abs() < 1e-4, "at {player}: {}", view_model_fov(&s, player));
    }
    // Zoomed: the game's own turn scale wins over the FOV rule (as Combat
    // Arms' scopes), whatever the field of view the camera shows.
    let mouse = MouseSettings::default();
    for (id, level, zoomed) in zooms() {
        let m = 1.0 / zoomed.sensitivity.unwrap();
        assert_eq!(mouse.zoomed_scale(Some(&zoomed)), 1.0 / m, "{id} {level}");
    }
}
