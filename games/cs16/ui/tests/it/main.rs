//! Counter-Strike 1.6's front end's tests: what only the client shows (its
//! registration, the HUD's nodes from a frame; the layout and fades are
//! `hud`'s unit tests).

#[test]
fn the_client_is_the_game_s_with_its_hud() {
    assert_eq!(cs16_ui::party::CLIENT.game, cs16::game::GAME);
    assert!(cs16_ui::party::CLIENT.hud.is_some(), "CS 1.6's HUD (goldsrc M3)");
    assert_eq!(cs16::game::parts::PARTS.hud, Some(cs16::game::GAME));
}

mod hud_nodes {
    //! The HUD's nodes from a frame (specs/cs16/hud.md): with the
    //! pictures here, each draw is a node at its pixels, tinted in the HUD
    //! colour and added; without them only the bars show.

    use bevy::prelude::*;
    use chimera_client::client::hud_text::HudGlyphMaterial;
    use cs16_ui::hud::{Ammo, Cs16HudState, Fades, FillSlot, Frame, HudPictures, Pain, SpriteSlot, ammo_key, spawn_root, sync};

    fn app(pictures: HudPictures, frame: Frame, screen: (i32, i32, f32)) -> App {
        let mut app = App::new();
        app.insert_resource(pictures)
            .insert_resource(Cs16HudState { frame, screen })
            .init_resource::<Fades>()
            .init_resource::<Pain>()
            .init_resource::<Assets<HudGlyphMaterial>>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, sync);
        // The pool grows on the first frame, is filled on the next.
        app.update();
        app.update();
        app
    }

    fn pictures() -> HudPictures {
        let mut p = HudPictures::default();
        let mut add = |k: &str, w: u32, h: u32| {
            p.0.insert(k.to_string(), (Handle::<Image>::default(), UVec2::new(w, h)));
        };
        for d in 0..10 {
            add(&format!("number_{d}"), 20, 25);
        }
        for k in ["cross", "suit_empty", "suit_full", "suithelmet_empty", "suithelmet_full"] {
            add(k, 24, 24);
        }
        add(&ammo_key("cs16:weapon_ak47"), 24, 24);
        p
    }

    fn ak() -> Frame {
        Frame { health: Some(100), armour: (50, false), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip: 30, reserve: 90 } }
    }

    /// Shown sprite nodes: (left, top, width, height) in logical pixels and
    /// the material's colour.
    fn shown(app: &mut App) -> Vec<([f32; 4], LinearRgba)> {
        let w = app.world_mut();
        let mut q = w.query::<(&SpriteSlot, &Node, &Visibility, &MaterialNode<HudGlyphMaterial>)>();
        let mut out: Vec<_> = q
            .iter(w)
            .filter(|(_, _, v, _)| **v != Visibility::Hidden)
            .map(|(s, n, _, m)| {
                let px = |v: Val| if let Val::Px(p) = v { p } else { f32::NAN };
                (s.0, [px(n.left), px(n.top), px(n.width), px(n.height)], m.0.clone())
            })
            .collect();
        out.sort_by_key(|s| s.0);
        let materials = w.resource::<Assets<HudGlyphMaterial>>();
        out.into_iter().map(|(_, r, m)| (r, materials.get(&m).unwrap().params.color)).collect()
    }

    #[test]
    fn the_status_rows_are_nodes_at_their_pixels() {
        let mut app = app(pictures(), ak(), (640, 480, 1.0));
        let nodes = shown(&mut app);
        // cross, 3 health digits, empty suit, lit suit, 2 armour digits, 2 +
        // 2 ammo digits, ammo icon.
        assert_eq!(nodes.len(), 13);
        assert_eq!(nodes[0].0, [12.0, 443.0, 24.0, 24.0]);
        // The lit suit: its lower 12 rows.
        assert_eq!(nodes[5].0, [128.0, 451.0, 24.0, 12.0]);
        assert_eq!(nodes[12].0, [598.0, 445.0, 24.0, 24.0]);
        // Tinted in the HUD colour at the dim alpha, added.
        let c = Color::srgba_u8(255, 160, 0, 100).to_linear();
        assert_eq!(nodes[0].1, c);
        let w = app.world_mut();
        let fills = w.query::<(&FillSlot, &Visibility)>().iter(w).filter(|(_, v)| **v != Visibility::Hidden).count();
        assert_eq!(fills, 2, "health's bar and the ammunition's");
    }

    #[test]
    fn the_pixels_are_the_window_s_real_ones() {
        // At scale factor 2 a 24-pixel icon is 12 logical pixels.
        let mut app = app(pictures(), ak(), (1280, 960, 2.0));
        assert_eq!(shown(&mut app)[0].0, [6.0, 461.5, 12.0, 12.0]);
    }

    #[test]
    fn without_pictures_no_sprite_shows() {
        let mut app = app(HudPictures::default(), ak(), (640, 480, 1.0));
        assert!(shown(&mut app).is_empty());
    }
}
