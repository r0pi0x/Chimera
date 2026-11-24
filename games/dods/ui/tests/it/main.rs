//! DoD:S's front end's tests: what only the client shows (its
//! registration, the HUD's nodes from a frame; the frame is `hud`'s unit
//! test).

#[test]
fn the_client_is_dods_with_its_hud() {
    assert_eq!(dods_ui::party::CLIENT.game, dods::game::GAME);
    assert!(dods_ui::party::CLIENT.hud.is_some(), "DoD:S's HUD (M3b)");
}

mod hud_nodes {
    //! The HUD's nodes from a frame (specs/dods/hud.md): with the
    //! install's pictures the magazine shows its rounds' share and the
    //! spare count, the silhouette its lost share in red, the stamina its
    //! lit slices; without them the weapon is written.

    use bevy::prelude::*;
    use dods_ui::hud::{DodsHudState, Frame, HudPictures, Part, WeaponImages, colours, spawn_root, sync};

    fn app(pictures: HudPictures, frame: Frame) -> App {
        let mut app = App::new();
        app.insert_resource(pictures).insert_resource(DodsHudState { frame }).add_systems(Startup, spawn_root).add_systems(Update, sync);
        app.update();
        app
    }

    fn thompson() -> Frame {
        Frame {
            health: Some(40),
            stamina: Some(0.3),
            weapon_id: "dods:weapon_thompson".into(),
            weapon: "Thompson".into(),
            ammo: Some((15, 210)),
            class: Some("dods:us_assault".into()),
        }
    }

    fn pictures() -> HudPictures {
        let pic = |w: f32, h: f32| Some((Handle::<Image>::default(), Vec2::new(w, h)));
        let mut p = HudPictures::default();
        p.weapons.insert(
            "dods:weapon_thompson".into(),
            WeaponImages {
                weapon: pic(350.0, 122.0),
                glow: pic(350.0, 122.0),
                ammo: pic(36.0, 184.0),
                ammo_empty: pic(36.0, 184.0),
                clip: Some(dods::game::hud::ClipFill { height: 184.0, base: 2.0, bullet: 6.0 }),
                clip_size: 30,
            },
        );
        p.classes.insert("dods:us_assault".into(), (pic(64.0, 128.0), pic(64.0, 128.0)));
        p
    }

    fn part<'a>(app: &'a mut App, which: Part) -> (Option<ImageNode>, Option<String>, Visibility, Option<Color>) {
        let w = app.world_mut();
        let mut q = w.query::<(&Part, Option<&ImageNode>, Option<&Text>, Option<&Visibility>, Option<&BackgroundColor>)>();
        let (_, img, text, vis, bg) = q.iter(w).find(|(p, ..)| **p == which).unwrap();
        (img.cloned(), text.map(|t| t.0.clone()), vis.copied().unwrap_or(Visibility::Inherited), bg.map(|b| b.0))
    }

    #[test]
    fn the_strip_shows_the_rounds_health_and_stamina() {
        let mut app = app(pictures(), thompson());
        let (img, _, vis, _) = part(&mut app, Part::MagazineFull);
        assert_eq!(vis, Visibility::Inherited);
        // 15 rounds: 2 + 15 x 6 = 92 of 184 texels, from the bottom.
        let rect = img.unwrap().rect.unwrap();
        assert_eq!((rect.min.y, rect.max.y), (92.0, 184.0));
        assert_eq!(part(&mut app, Part::Count).1.as_deref(), Some("7"));
        assert_eq!(part(&mut app, Part::CountX).1.as_deref(), Some("x"));
        // 40 health: 60 % lost, red from the top.
        let rect = part(&mut app, Part::SilhouetteRed).0.unwrap().rect.unwrap();
        assert!((rect.max.y - 128.0 * 0.6).abs() < 1e-3);
        // 30 stamina: 15 of 53 slices lit, in the low colours.
        assert_eq!(part(&mut app, Part::Slice(14)).3, Some(colours::BAR_ACTIVE_LOW));
        assert_eq!(part(&mut app, Part::Slice(15)).3, Some(colours::BAR_INACTIVE_LOW));
        assert_eq!(part(&mut app, Part::WeaponText).2, Visibility::Hidden);
    }

    #[test]
    fn without_pictures_the_weapon_is_written() {
        let mut app = app(HudPictures::default(), thompson());
        let (_, text, vis, _) = part(&mut app, Part::WeaponText);
        assert_eq!((text.as_deref(), vis), (Some("Thompson  15 | 210"), Visibility::Inherited));
        assert_eq!(part(&mut app, Part::MagazineFull).2, Visibility::Hidden);
        assert_eq!(part(&mut app, Part::Silhouette).2, Visibility::Hidden);
    }
}
