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
    use cs16_ui::hud::{Ammo, Crosshair, CrosshairLook, Cs16HudState, Fades, FillSlot, Frame, HudPictures, Menu, MenuWeapon, Pain, SpriteSlot, ammo_key, menu_key, menu_lit_key, spawn_root, sync};

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
        let crosshair = Crosshair { base: 4, step: 4.0, look: CrosshairLook::default() };
        let f = Frame { crosshair: Some(crosshair), ..ak() };
        fades.step(&f, 0.0);
        let mut big = app_with(pictures(), f, (1920, 1080, 1.0), fades);
        let nodes = shown(&mut big);
        // AK-47 at rest on 1920 x 1080: scale 3, gap 12, lines 15 long.
        let lines: Vec<[f32; 4]> = nodes.iter().map(|n| n.0).filter(|r| r[2] <= 15.0 && (r[2] == 1.0 || r[3] == 1.0)).collect();
        assert_eq!(lines, vec![[934.0, 540.0, 15.0, 1.0], [972.0, 540.0, 15.0, 1.0], [960.0, 514.0, 1.0, 15.0], [960.0, 552.0, 1.0, 15.0]]);
        // The default colour at the starting alpha (200, 2 more the first frame).
        let green = Color::srgba_u8(50, 250, 50, 202).to_linear();
        assert!(nodes.iter().any(|n| n.1 == green));
        // Without the install's pictures the lines still show.
        let mut bare = HudPictures::default();
        bare.1 = Some(Handle::<Image>::default());
        let mut app = app(bare, Frame { crosshair: Some(crosshair), health: Some(100), ..Default::default() }, (640, 480, 1.0));
        assert_eq!(shown(&mut app).len(), 4);
        // Blended (cl_crosshair_translucent 0): filled boxes, not sprites.
        let look = CrosshairLook { translucent: 0, ..CrosshairLook::default() };
        let mut app = app_with(HudPictures::default(), Frame { crosshair: Some(Crosshair { look, ..crosshair }), health: Some(100), ..Default::default() }, (640, 480, 1.0), Fades::default());
        let w = app.world_mut();
        assert_eq!(w.query::<(&FillSlot, &Visibility)>().iter(w).filter(|(_, v)| **v != Visibility::Hidden).count(), 5, "four lines and health's bar");
    }

    /// The selection's slot numbers, the open slot's pictures with the
    /// chosen one lit under the highlight, boxes under the closed slots.
    #[test]
    fn the_weapon_menu_is_slot_numbers_boxes_and_pictures() {
        let weapon = |id: &str, empty| MenuWeapon { id: id.into(), empty, ..Default::default() };
        let menu = Menu {
            slots: vec![
                vec![weapon("cs16:weapon_ak47", false), weapon("cs16:weapon_usp", true)],
                vec![],
                vec![weapon("cs16:weapon_knife", false)],
            ],
            chosen: Some((0, 1)),
        };
        let f = Frame { health: Some(100), menu: Some(menu), ..Default::default() };
        let mut app = app(pictures(), f, (640, 480, 1.0));
        let nodes: Vec<[f32; 4]> = shown(&mut app).iter().map(|n| n.0).filter(|r| r[1] < 140.0).collect();
        // Right of the radar (128 + 10): bucket 1 at 138, then the open
        // slot's 170 + 5; the AK at the top of the open list (30), the lit
        // USP under it with the selection over it; the knife's box under
        // slot 3.
        assert!(nodes.contains(&[138.0, 10.0, 20.0, 20.0]), "bucket 1");
        assert!(nodes.contains(&[313.0, 10.0, 20.0, 20.0]), "bucket 2 after the open slot");
        assert!(nodes.contains(&[138.0, 30.0, 170.0, 45.0]), "the first weapon");
        assert_eq!(nodes.iter().filter(|r| **r == [138.0, 80.0, 170.0, 45.0]).count(), 2, "the lit one and the selection");
        let w = app.world_mut();
        let boxes: Vec<(f32, f32)> = w
            .query::<(&SpriteSlot, &Node, &Visibility)>()
            .iter(w)
            .filter(|(_, _, v)| **v != Visibility::Hidden)
            .filter_map(|(_, n, _)| if let (Val::Px(l), Val::Px(t), Val::Px(bw)) = (n.left, n.top, n.width) { (bw == 20.0 && t == 30.0).then_some((l, t)) } else { None })
            .collect();
        assert!(boxes.contains(&(338.0, 30.0)), "the knife's box under slot 3: {boxes:?}");
    }

    /// At 320 x 240 the menu's columns start at 74, its pictures at y 22.
    #[test]
    fn the_320_menu_has_its_own_places() {
        let weapon = |id: &str, reserve| MenuWeapon { id: id.into(), reserve, most: 90, ..Default::default() };
        let menu = Menu { slots: vec![vec![weapon("cs16:weapon_ak47", 45), weapon("cs16:weapon_usp", 0)]], chosen: Some((0, 0)) };
        let mut p = pictures_320();
        let small = p.0.get_mut(&320).unwrap();
        for k in ["bucket1", "bucket2", "bucket3", "bucket4", "bucket5", "bucket0"] {
            small.insert(k.into(), (Handle::<Image>::default(), UVec2::splat(12)));
        }
        for id in ["cs16:weapon_ak47", "cs16:weapon_usp"] {
            small.insert(menu_key(id), (Handle::<Image>::default(), UVec2::new(80, 20)));
            small.insert(menu_lit_key(id), (Handle::<Image>::default(), UVec2::new(80, 20)));
        }
        small.insert("selection".into(), (Handle::<Image>::default(), UVec2::new(80, 20)));
        let f = Frame { health: Some(100), menu: Some(menu), ..Default::default() };
        let mut app = app(p, f, (320, 240, 1.0));
        let nodes: Vec<[f32; 4]> = shown(&mut app).iter().map(|n| n.0).filter(|r| r[1] < 80.0).collect();
        assert!(nodes.contains(&[74.0, 10.0, 12.0, 12.0]), "bucket 1");
        assert!(nodes.contains(&[74.0, 22.0, 80.0, 20.0]), "the first picture");
        assert!(nodes.contains(&[74.0, 47.0, 80.0, 20.0]), "the second, 25 apart");
        assert!(nodes.contains(&[79.0, 22.0, 5.0, 2.0]), "the bar's filled part, 10 x 2 from x + 5");
    }

    #[test]
    fn the_round_timer_starts_at_the_screen_s_centre() {
        let f = Frame { health: Some(100), timer: Some(164), ..Default::default() };
        let mut app = app(pictures(), f, (640, 480, 1.0));
        let nodes = shown(&mut app);
        // `2:44` at 640 x 480: the stopwatch's left edge on the centre
        // (320, 442), the minutes from 326, the colon's squares at 372
        // (alpha 100), the seconds from 378.
        assert!(nodes.iter().any(|n| n.0 == [320.0, 442.0, 24.0, 24.0]), "stopwatch");
        assert!(nodes.iter().any(|n| n.0 == [346.0, 442.0, 20.0, 25.0]), "minutes");
        assert!(nodes.iter().any(|n| n.0 == [378.0, 442.0, 20.0, 25.0]), "seconds, tens");
        assert!(nodes.iter().any(|n| n.0 == [398.0, 442.0, 20.0, 25.0]), "seconds, ones");
        let squares: Vec<[f32; 4]> = nodes.iter().map(|n| n.0).filter(|r| r[2] == 2.0 && r[3] == 2.0).collect();
        assert_eq!(squares, vec![[372.0, 448.0, 2.0, 2.0], [372.0, 461.0, 2.0, 2.0]]);
        // All dim: the HUD colour at alpha 100.
        let dim = Color::srgba_u8(255, 160, 0, 100).to_linear();
        assert!(nodes.iter().filter(|n| n.0[1] == 442.0 || n.0[2] == 2.0).all(|n| n.1 == dim));
    }

    /// 320 x 240 with `2:44`: stopwatch (160, 216), `2` at 176, the colon's
    /// squares at x 192, y 220 and 228, the seconds at 196 and 208.
    #[test]
    fn the_320_timer_has_its_own_places() {
        let f = Frame { health: Some(100), timer: Some(164), ..Default::default() };
        let mut app = app(pictures_320(), f, (320, 240, 1.0));
        let nodes = shown(&mut app);
        assert!(nodes.iter().any(|n| n.0 == [160.0, 216.0, 16.0, 16.0]), "stopwatch");
        assert!(nodes.iter().any(|n| n.0 == [176.0, 216.0, 12.0, 16.0]), "minutes");
        assert!(nodes.iter().any(|n| n.0 == [196.0, 216.0, 12.0, 16.0]) && nodes.iter().any(|n| n.0 == [208.0, 216.0, 12.0, 16.0]), "seconds");
        let squares: Vec<[f32; 4]> = nodes.iter().map(|n| n.0).filter(|r| r[2] == 2.0 && r[3] == 2.0).collect();
        assert_eq!(squares, vec![[192.0, 220.0, 2.0, 2.0], [192.0, 228.0, 2.0, 2.0]]);
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
    use chimera_client::client::weapon_menu::WeaponMenu;
    use chimera_core::core::{Health, LocalPlayer, MovementState, SimClock, Velocity};
    use chimera_sim::{
        mode::{Match, Phase},
        weapon::{AltModes, Inventory, Magazine, Weapon},
    };
    use cs16_ui::hud::{Cs16HudState, Fades, FastSwitch, collect, keep_open};

    fn gun(world: &mut World, id: &'static str, slot: u8, clip: u32) -> Entity {
        let mag = Magazine { clip, size: 30, reserve: 0, reserve_max: 90, reload_time: 2.0, reload_while_held: false, auto_reload: false };
        world.spawn((Weapon { id, slot, owner: None, draw_time: 0.0, max_speed: None }, mag)).id()
    }

    fn world() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<Cs16HudState>()
            .init_resource::<Fades>()
            .init_resource::<WeaponMenu>()
            .insert_resource(Match { phase: Phase::Live, round_ends: Some(100.0), ..default() })
            .insert_resource(SimClock { now: 40.5, ..default() })
            .add_systems(Update, collect);
        let ak = gun(app.world_mut(), "cs16:weapon_ak47", 0, 30);
        let usp = gun(app.world_mut(), "cs16:weapon_usp", 1, 0);
        let mut inv = Inventory::default();
        inv.weapons = vec![ak, usp];
        inv.active = Some(ak);
        app.world_mut().spawn((LocalPlayer, Health::default(), inv, MovementState { on_ground: true, ..default() }, Velocity::default()));
        (app, ak, usp)
    }

    #[test]
    fn the_timer_counts_the_round_s_whole_seconds_down() {
        let (mut app, ..) = world();
        app.update();
        // 59.5 s left, truncated.
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, Some(59));
        // A limit too far off to show (the sandbox's: over a day) is none.
        app.world_mut().resource_mut::<Match>().round_ends = Some(1.0e7);
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, None);
        app.world_mut().resource_mut::<Match>().round_ends = Some(100.0);
        app.world_mut().resource_mut::<Match>().phase = Phase::Waiting;
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.timer, None);
    }

    #[test]
    fn the_crosshair_follows_the_player_s_movement_and_hides_when_dead() {
        let (mut app, ak, _) = world();
        let me = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(app.world()).unwrap();
        app.update();
        let base = |app: &App| app.world().resource::<Cs16HudState>().frame.crosshair.map(|c| c.base);
        assert_eq!(base(&app), Some(4), "the AK-47 at rest");
        // Running at 250 units a second: 6; in the air: 8.
        app.world_mut().entity_mut(me).insert(Velocity(Vec3::X * 250.0 * 0.0254));
        app.update();
        assert_eq!(base(&app), Some(6));
        app.world_mut().get_mut::<MovementState>(me).unwrap().on_ground = false;
        app.update();
        assert_eq!(base(&app), Some(8));
        // A shot opens the gap by the step (4), once per frame.
        app.world_mut().get_mut::<MovementState>(me).unwrap().on_ground = true;
        app.world_mut().entity_mut(me).insert(Velocity::default());
        app.update();
        let gap = app.world().resource::<Fades>().gap;
        app.world_mut().get_mut::<Magazine>(ak).unwrap().clip = 27;
        app.update();
        assert_eq!(app.world().resource::<Fades>().gap, (gap + 4.0).min(15.0));
        // The silencer's mode doesn't change an AK; a Glock's semi-automatic mode widens it.
        app.world_mut().get_mut::<Magazine>(ak).unwrap().clip = 30;
        app.world_mut().entity_mut(ak).insert(AltModes::new(2, 1.0, false));
        app.update();
        assert_eq!(base(&app), Some(4));
        app.world_mut().entity_mut(me).insert(chimera_core::core::Dead { since: 0.0 });
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.crosshair, None);
    }

    #[test]
    fn a_sniper_rifle_has_no_crosshair() {
        let (mut app, ..) = world();
        let awp = gun(app.world_mut(), "cs16:weapon_awp", 0, 10);
        let me = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(app.world()).unwrap();
        app.world_mut().get_mut::<Inventory>(me).unwrap().active = Some(awp);
        app.update();
        assert_eq!(app.world().resource::<Cs16HudState>().frame.crosshair, None);
    }

    /// No timeout: the selection stays open however long it is untouched.
    #[test]
    fn the_menu_has_no_timeout_and_closes_on_death() {
        let (mut app, _, usp) = world();
        app.init_resource::<FastSwitch>().add_systems(Update, keep_open.before(collect));
        {
            let mut m = app.world_mut().resource_mut::<WeaponMenu>();
            m.choosing = true;
            m.shown_for = Some(0.0);
            m.highlighted = Some(usp);
        }
        for _ in 0..5 {
            // The shell would add the frame's time (10 s a frame here).
            app.world_mut().resource_mut::<WeaponMenu>().shown_for.as_mut().map(|t| *t += 10.0);
            app.update();
        }
        assert!(app.world().resource::<WeaponMenu>().choosing);
        assert!(app.world().resource::<Cs16HudState>().frame.menu.is_some());
        let me = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(me).insert(chimera_core::core::Dead { since: 0.0 });
        app.update();
        assert!(!app.world().resource::<WeaponMenu>().choosing);
    }

    #[test]
    fn fast_switch_is_off_by_default() {
        assert_eq!(FastSwitch::default().0, 0);
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
    }
}
