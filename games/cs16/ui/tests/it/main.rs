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
    use cs16_ui::hud::{Ammo, Cs16HudState, Fades, FillSlot, Frame, HudPictures, Menu, MenuWeapon, Pain, SpriteSlot, ammo_key, menu_key, menu_lit_key, spawn_root, sync};

    fn app(pictures: HudPictures, frame: Frame, screen: (i32, i32, f32)) -> App {
        app_with(pictures, frame, screen, Fades::default())
    }

    fn app_with(pictures: HudPictures, frame: Frame, screen: (i32, i32, f32), fades: Fades) -> App {
        let mut app = App::new();
        app.insert_resource(pictures)
            .insert_resource(Cs16HudState { frame, screen })
            .insert_resource(fades)
            .init_resource::<Pain>()
            .init_resource::<Assets<HudGlyphMaterial>>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, sync);
        // The pool grows on the first frame, is filled on the next.
        app.update();
        app.update();
        app
    }

    /// The 640 layout's pictures at their real sizes (and the white pixel).
    fn pictures() -> HudPictures {
        let mut p = HudPictures::default();
        let layout = p.0.entry(640).or_default();
        let mut add = |k: &str, w: u32, h: u32| {
            layout.insert(k.to_string(), (Handle::<Image>::default(), UVec2::new(w, h)));
        };
        for d in 0..10 {
            add(&format!("number_{d}"), 20, 25);
        }
        for k in ["cross", "suit_empty", "suit_full", "suithelmet_empty", "suithelmet_full", "stopwatch"] {
            add(k, 24, 24);
        }
        for k in ["bucket1", "bucket2", "bucket3", "bucket4", "bucket5", "bucket0"] {
            add(k, 20, 20);
        }
        add("selection", 170, 45);
        for id in ["cs16:weapon_ak47", "cs16:weapon_usp", "cs16:weapon_knife"] {
            add(&ammo_key(id), 24, 24);
            add(&menu_key(id), 170, 45);
            add(&menu_lit_key(id), 170, 45);
        }
        p.1 = Some(Handle::<Image>::default());
        p
    }

    /// The 320 layout's: 12 x 16 digits, 16 pixel icons.
    fn pictures_320() -> HudPictures {
        let mut p = pictures();
        let mut small = std::collections::BTreeMap::new();
        for d in 0..10 {
            small.insert(format!("number_{d}"), (Handle::<Image>::default(), UVec2::new(12, 16)));
        }
        for k in ["cross", "suit_empty", "suit_full", "suithelmet_empty", "suithelmet_full", "stopwatch"] {
            small.insert(k.to_string(), (Handle::<Image>::default(), UVec2::splat(16)));
        }
        p.0.insert(320, small);
        p
    }

    fn ak() -> Frame {
        Frame { health: Some(100), armour: (50, false), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip: 30, reserve: 90 }, ..Default::default() }
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

    /// The crosshair's four lines are added nodes at the screen's centre,
    /// the gap from the frame's fade, whatever the installed pictures.
    #[test]
    fn the_crosshair_is_four_added_lines_around_the_centre() {
        let mut fades = Fades::default();
        let f = Frame { crosshair: Some(10.0), ..ak() };
        fades.step(&f, 0.0);
        let mut big = app_with(pictures(), f, (1920, 1080, 1.0), fades);
        let nodes = shown(&mut big);
        // Gap 4 + 10; length 9 at this width.
        let lines: Vec<[f32; 4]> = nodes.iter().map(|n| n.0).filter(|r| r[2] <= 9.0 && (r[2] == 1.0 || r[3] == 1.0)).collect();
        assert_eq!(lines, vec![[937.0, 540.0, 9.0, 1.0], [974.0, 540.0, 9.0, 1.0], [960.0, 517.0, 1.0, 9.0], [960.0, 554.0, 1.0, 9.0]]);
        let green = Color::srgba_u8(50, 250, 50, 255).to_linear();
        assert!(nodes.iter().any(|n| n.1 == green));
        // Without the install's pictures the lines still show.
        let mut bare = HudPictures::default();
        bare.1 = Some(Handle::<Image>::default());
        let mut app = app(bare, Frame { crosshair: Some(0.0), health: Some(100), ..Default::default() }, (640, 480, 1.0));
        assert_eq!(shown(&mut app).len(), 4);
    }

    /// The selection's slot numbers, the open slot's pictures with the
    /// chosen one lit under the highlight, boxes under the closed slots.
    #[test]
    fn the_weapon_menu_is_slot_numbers_boxes_and_pictures() {
        let weapon = |id: &str, empty| MenuWeapon { id: id.into(), empty };
        let menu = Menu {
            slots: vec![
                vec![weapon("cs16:weapon_ak47", false), weapon("cs16:weapon_usp", true)],
                vec![],
                vec![weapon("cs16:weapon_knife", false)],
            ],
            chosen: Some((0, 1)),
            fade: 1.0,
        };
        let f = Frame { health: Some(100), menu: Some(menu), ..Default::default() };
        let mut app = app(pictures(), f, (640, 480, 1.0));
        let nodes: Vec<[f32; 4]> = shown(&mut app).iter().map(|n| n.0).filter(|r| r[1] < 140.0).collect();
        // Buckets 1..5 at 10 + the open slot's 170 wide cell, then 20 + 5
        // each; the AK at the top of the open list (40), the lit USP under
        // it with the selection over it; the knife's box under slot 3.
        assert!(nodes.contains(&[10.0, 10.0, 20.0, 20.0]), "bucket 1");
        assert!(nodes.contains(&[185.0, 10.0, 20.0, 20.0]), "bucket 2 after the open slot");
        assert!(nodes.contains(&[10.0, 40.0, 170.0, 45.0]), "the first weapon");
        assert_eq!(nodes.iter().filter(|r| **r == [10.0, 90.0, 170.0, 45.0]).count(), 2, "the lit one and the selection");
        let w = app.world_mut();
        let boxes: Vec<(f32, f32)> = w
            .query::<(&FillSlot, &Node, &Visibility)>()
            .iter(w)
            .filter(|(_, _, v)| **v != Visibility::Hidden)
            .filter_map(|(_, n, _)| if let (Val::Px(l), Val::Px(t)) = (n.left, n.top) { Some((l, t)) } else { None })
            .collect();
        assert!(boxes.contains(&(210.0, 40.0)), "the knife's box under slot 3: {boxes:?}");
    }

    #[test]
    fn the_round_timer_is_centred_on_the_screen() {
        let f = Frame { health: Some(100), timer: Some(165), ..Default::default() };
        let mut app = app(pictures(), f, (640, 480, 1.0));
        let nodes = shown(&mut app);
        // Stopwatch (24) + `2` + colon (10) + `4` `5`: 24 + 3 x 20 + 10 = 94
        // wide, from 320 - 47 = 273, on the health row's cross height.
        assert!(nodes.iter().any(|n| n.0 == [273.0, 443.0, 24.0, 24.0]));
        assert!(nodes.iter().any(|n| n.0 == [297.0, 448.0, 20.0, 25.0]), "minutes");
        assert!(nodes.iter().any(|n| n.0 == [327.0, 448.0, 20.0, 25.0]), "seconds, tens");
    }

    /// Below 640 wide the 320 layout's pictures lay the HUD out; a wider
    /// screen the 640 ones.
    #[test]
    fn a_narrow_screen_uses_the_320_layout() {
        let mut app = app(pictures_320(), ak(), (320, 240, 1.0));
        let nodes = shown(&mut app);
        // Font height 16: the baseline 240 - 24 = 216; the cross at 8.
        assert_eq!(nodes[0].0, [8.0, 216.0, 16.0, 16.0]);
        // Health digits from 16 + 6 = 22 in 12 pixel steps.
        assert_eq!(nodes[1].0[0], 22.0);
        assert_eq!(nodes[2].0[0], 34.0);
        let mut wide = app_wide();
        assert_eq!(shown(&mut wide)[0].0, [12.0, 443.0, 24.0, 24.0]);
    }

    fn app_wide() -> App {
        app(pictures_320(), ak(), (640, 480, 1.0))
    }

    #[test]
    fn without_pictures_no_sprite_shows() {
        let mut app = app(HudPictures::default(), ak(), (640, 480, 1.0));
        assert!(shown(&mut app).is_empty());
    }
}

mod collected {
    //! `collect`: the frame from a local player's world (the weapon in
    //! hand's spread, the selection's state, the round's clock).

    use bevy::prelude::*;
    use chimera_client::client::weapon_menu::{WeaponMenu, WeaponMenuTimes};
    use chimera_core::core::{Health, LocalPlayer, SimClock};
    use chimera_sim::{
        mode::{Match, Phase},
        weapon::{Hitscan, Inventory, Magazine, SpreadShape, Weapon},
    };
    use cs16_ui::hud::{Cs16HudState, Fades, collect};

    fn gun(world: &mut World, id: &'static str, slot: u8, spread: f32, clip: u32) -> Entity {
        let hitscan = Hitscan {
            range: 100.0,
            pellets: 1,
            spread: SpreadShape::Disc { inaccuracy: spread, spread: 0.0 },
            punch_scale: 0.0,
            tracer_every: 0,
            redrawn_through_characters: false,
            odd_box: None,
            centre_first: false,
        };
        let mag = Magazine { clip, size: 30, reserve: 0, reserve_max: 90, reload_time: 2.0, reload_while_held: false, auto_reload: false };
        world.spawn((Weapon { id, slot, owner: None, draw_time: 0.0, max_speed: None }, hitscan, mag)).id()
    }

    fn world() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<Cs16HudState>()
            .init_resource::<Fades>()
            .init_resource::<WeaponMenu>()
            .init_resource::<WeaponMenuTimes>()
            .insert_resource(Match { phase: Phase::Live, round_ends: Some(100.0), ..default() })
            .insert_resource(SimClock { now: 40.5, ..default() })
            .add_systems(Update, collect);
        let ak = gun(app.world_mut(), "cs16:weapon_ak47", 0, 0.0, 30);
        let usp = gun(app.world_mut(), "cs16:weapon_usp", 1, 0.0, 0);
        let mut inv = Inventory::default();
        inv.weapons = vec![ak, usp];
        inv.active = Some(ak);
        app.world_mut().spawn((LocalPlayer, Health::default(), inv));
        (app, ak, usp)
    }

    #[test]
    fn the_timer_counts_the_round_s_seconds_up() {
        let (mut app, ..) = world();
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, Some(60));
        // A limit too far off to show (the sandbox's) is none.
        app.world_mut().resource_mut::<Match>().round_ends = Some(1.0e7);
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, None);
        app.world_mut().resource_mut::<Match>().round_ends = Some(100.0);
        app.world_mut().resource_mut::<Match>().phase = Phase::Waiting;
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, None);
    }

    #[test]
    fn the_crosshair_follows_the_held_gun_s_spread_and_hides_when_dead() {
        let (mut app, ak, _) = world();
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.crosshair, Some(0.0));
        // 0.1 tangent units at a 1.5 rad view (the fallback) on 480 rows.
        app.world_mut().get_mut::<Hitscan>(ak).unwrap().spread = SpreadShape::Disc { inaccuracy: 0.1, spread: 0.0 };
        app.update();
        let gap = app.world().resource::<Cs16HudState>().frame.crosshair.unwrap();
        assert!((gap - 0.1 / (0.75f32).tan() * 240.0).abs() < 0.01, "{gap}");
        let me = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(me).insert(chimera_core::core::Dead { since: 0.0 });
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.crosshair, None);
    }

    #[test]
    fn the_menu_lists_held_weapons_by_slot_while_choosing() {
        let (mut app, _, usp) = world();
        app.update();
        assert!(app.world().resource::<Cs16HudState>().frame.menu.is_none());
        {
            let mut m = app.world_mut().resource_mut::<WeaponMenu>();
            m.choosing = true;
            m.shown_for = Some(0.5);
            m.highlighted = Some(usp);
        }
        app.update();
        let menu = app.world().resource::<Cs16HudState>().frame.menu.clone().expect("the selection is open");
        assert_eq!(menu.chosen, Some((1, 0)));
        assert_eq!(menu.slots.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 1]);
        // The USP has no rounds: its picture is the empty colour's.
        assert!(!menu.slots[0][0].empty && menu.slots[1][0].empty);
        assert_eq!(menu.fade, 1.0);
    }
}
