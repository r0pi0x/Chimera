//! TF2's bodies and HUD look from the user's install (M3): each M2 gun's
//! class's player model as a body (specs/tf2/player_animation.md 1, 3, 4,
//! 8): its include's sequences under the `ACT_MP_*` activities by kind,
//! the four pose parameters by name, the head's hitbox in group 1 on
//! `bip_head`, a RED and a BLU skin; the animation state on it (test cases
//! 2, 13, 14: a Scout running forward plays `run_PRIMARY` with `move_x`
//! 1); and the HUD's look (specs/tf2/hud.md 2): the scheme's colours, its
//! fonts with their files, the pictures and the kill icons' cells. Passes
//! with a note without the install.

use bevy::prelude::*;
use chimera_core::core::Hitgroup;
use chimera_world::map::{GameKit, KitLoad, anim::Animator};
use source_engine::{
    content::{material::MaterialLoader, surfaceprops::SurfaceProps},
    player_anim::Inputs,
};
use tf2::game::{bodies, hud, player_anim::Tf2PlayerAnim};

use super::maps::install_mount;

fn bodies_kit() -> Option<GameKit> {
    let mount = install_mount()?;
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let (mut kit, mut warnings) = (GameKit::default(), Vec::new());
    bodies::add_bodies(&mut materials, &surfaces, KitLoad::Players(&["tf2:"]), &mut kit, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    Some(kit)
}

#[test]
fn each_class_body_has_its_sequences_poses_head_and_skins() {
    let Some(kit) = bodies_kit() else { return };
    for (model, named) in [
        ("models/player/scout.mdl", true),
        ("models/player/soldier.mdl", false),
        ("models/player/heavy.mdl", true),
        ("models/player/engineer.mdl", true),
    ] {
        let body = kit
            .characters
            .iter()
            .find(|c| c.name.as_deref() == Some(model) || (!named && c.name.is_none()))
            .unwrap_or_else(|| panic!("{model}: no body"));
        assert_eq!(body.game.as_deref(), Some("tf2"), "{model}");
        let set = body.animations.as_ref().unwrap_or_else(|| panic!("{model}: no animations"));
        for p in ["move_x", "move_y", "body_yaw", "body_pitch"] {
            assert!(set.param(p).is_some(), "{model}: no {p}");
        }
        for act in ["ACT_MP_STAND_PRIMARY", "ACT_MP_RUN_PRIMARY", "ACT_MP_CROUCH_PRIMARY", "ACT_MP_JUMP_START_PRIMARY", "ACT_MP_ATTACK_STAND_PRIMARY"] {
            let s = set.activity(act).unwrap_or_else(|| panic!("{model}: no {act}"));
            assert!(set.animations[set.sequences[s].anims[0]].frames > 1, "{model}: {act} is a stub");
        }
        let head = body.boxes.iter().find(|b| b.group == Hitgroup::Head).unwrap_or_else(|| panic!("{model}: no head box"));
        assert!(body.bones[head.bone].name.eq_ignore_ascii_case("bip_head"), "{model}: the head on {}", body.bones[head.bone].name);
        assert!(body.model.skins.len() >= 2, "{model}: RED and BLU skins, {}", body.model.skins.len());
        assert!(body.bones.iter().any(|b| b.name.eq_ignore_ascii_case("weapon_bone")), "{model}: weapon_bone");
    }
}

#[test]
fn a_running_scout_plays_its_run_grid_forward_2() {
    let Some(kit) = bodies_kit() else { return };
    let body = kit.characters.iter().find(|c| c.name.as_deref() == Some("models/player/scout.mdl")).unwrap();
    let set = body.animations.clone().unwrap();
    let mut a = Animator::new(set.clone());
    let mut s = Tf2PlayerAnim::default();
    let i = Inputs { velocity: Vec2::new(400.0, 0.0), on_ground: true, ..default() };
    for k in 0..10 {
        s.step(&mut a, &i, Some("tf2:tf_weapon_scattergun"), 0.015, k as f64 * 0.015);
    }
    let main = a.main.unwrap();
    assert!(set.sequences[main].name.eq_ignore_ascii_case("run_PRIMARY"), "{}", set.sequences[main].name);
    assert!((s.pose[0] - 1.0).abs() < 0.02 && s.pose[1].abs() < 0.02, "move {:?}", &s.pose[..2]);
    // Looking 60 degrees down: body_pitch -45 (test 14).
    let i = Inputs { eye_pitch: 60.0, on_ground: true, ..default() };
    s.step(&mut a, &i, Some("tf2:tf_weapon_scattergun"), 0.015, 1.0);
    assert_eq!(s.pose[3], -45.0);
    let main = a.main.unwrap();
    assert!(set.sequences[main].name.eq_ignore_ascii_case("stand_PRIMARY"), "{}", set.sequences[main].name);
}

#[test]
fn the_hud_look_has_tf2s_colours_fonts_pictures_and_kill_icons() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let look = hud::load(&mut materials).expect("the HUD's look");
    assert_eq!(look.game, "tf2");
    assert_eq!(look.colors.get("TanLight"), Some(&[235, 226, 202, 255]));
    assert_eq!(look.colors.get("HUDRedTeamSolid"), Some(&[180, 92, 77, 255]));
    for font in ["HudClassHealth", "HudFontGiantBold", "HudFontMediumSmall"] {
        let sizes = look.text_fonts.get(font).unwrap_or_else(|| panic!("no {font}"));
        let family = sizes[0].family.to_lowercase();
        assert!(look.fonts.contains_key(&format!("family:{family}")), "{font}: no file for {family} in {:?}", look.fonts.keys().collect::<Vec<_>>());
    }
    for p in ["health_bg", "health_over_bg", "health_color", "crosshairs"] {
        assert!(look.sprites.contains_key(p), "no {p}");
    }
    for icon in ["scattergun", "tf_projectile_rocket", "shotgun_primary", "minigun"] {
        assert!(look.sprites.contains_key(icon), "no kill icon {icon}");
        assert!(look.sprites.contains_key(&format!("{icon}/neg")), "no negative kill icon {icon}");
    }
    assert!(look.panels.contains_key("HudWeaponAmmo"));
}

/// The kill feed's icons (hud.md 3.6): each M2 gun's cell cut from the
/// install's `hud/d_images` (the Rocket Launcher's the rocket's), 96 x 32.
#[test]
fn the_guns_kill_icons_are_cut_from_the_sheet() {
    use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
    if install_mount().is_none() {
        return;
    }
    let mut app = App::new();
    app.init_resource::<Assets<Image>>().init_resource::<WeaponIcons>().add_systems(Update, tf2::ui::systems::register_kill_icons);
    app.update();
    let icons = app.world().resource::<WeaponIcons>();
    for gun in tf2::game::weapons::GUNS {
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
