use super::*;
    use super::*;

    const SCREEN: Vec2 = Vec2::new(1280.0, 720.0);

    /// A shade's alpha covers in linear light (Bevy's UI blending) as
    /// the asked alpha does in display space: black at 0.55 over a
    /// display-space grey leaves about 45% of it (the sRGB curve is not a
    /// pure 2.2 power: within 0.03), not ~70%; opaque stays opaque.
    #[test]
    fn display_alpha_darkens_as_a_display_space_blend_would() {
        let over = |under: f32, shade: Color| {
            let a = shade.alpha();
            let lin = LinearRgba::from(Color::srgb(under, under, under)).red * (1.0 - a);
            Srgba::from(LinearRgba::rgb(lin, lin, lin)).red
        };
        let shade = Color::srgba(0.0, 0.0, 0.0, 0.55);
        for under in [0.2, 0.5, 0.9] {
            assert!((over(under, display_alpha(shade)) - under * 0.45).abs() < 0.03, "{under}");
            assert!(over(under, shade) > under * 0.6, "Bevy's own blend: barely darker");
        }
        assert_eq!(display_alpha(Color::BLACK).alpha(), 1.0);
        assert_eq!(display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.0)).alpha(), 0.0);
    }

    /// A scroll bar's thumb: sized for the rows shown, placed for the
    /// first; dragged, its travel maps onto every first row there is, the
    /// ends past it clamped; none when everything shows.
    #[test]
    fn a_scroll_bars_thumb_maps_its_travel_onto_the_rows() {
        let (len, shown, room) = (100, 10, 200.0);
        assert_eq!(scroll_thumb(len, shown, 0, room), Some((0.0, 20.0)));
        assert_eq!(scroll_thumb(len, shown, 90, room), Some((180.0, 20.0)));
        assert_eq!(scroll_thumb(len, shown, 45, room), Some((90.0, 20.0)));
        assert_eq!(scroll_thumb(5, shown, 0, room), None);
        // A long list: no thinner than 10.
        assert_eq!(scroll_thumb(10_000, shown, 0, room).map(|t| t.1), Some(10.0));
        assert_eq!(scroll_first_at(len, shown, room, 20.0, 90.0), 45);
        assert_eq!(scroll_first_at(len, shown, room, 20.0, -50.0), 0);
        assert_eq!(scroll_first_at(len, shown, room, 20.0, 500.0), 90);
        assert_eq!(scroll_clamp(-3, len, shown), 0);
        assert_eq!(scroll_clamp(95, len, shown), 90);
        // Held at its top, drawn at 1.5 window pixels per scheme pixel:
        // the pointer 45 window pixels down is 30 scheme pixels: 15 rows.
        let held = ScrollHeld {
            thumb: ScrollThumb {
                owner: "menu",
                id: 0,
                len,
                shown,
                room,
                top: 0.0,
                thumb: 20.0,
            },
            grab: 300.0,
            scale: 1.5,
        };
        assert_eq!(held.first_at(300.0), 0);
        assert_eq!(held.first_at(345.0), 15);
        assert_eq!(held.first_at(1000.0), 90);
    }

    /// An open drop-down's catcher (the screen behind its list) takes a
    /// click whole: nothing under it, its own box included, sees the
    /// press (else a click on the box closed the list and opened it
    /// again).
    #[test]
    fn an_open_lists_catcher_blocks_the_click() {
        let mut app = App::new();
        let root = app.world_mut().spawn(Node::default()).id();
        let parent = app.world_mut().spawn(Node::default()).id();
        let mut commands_queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut commands_queue, app.world());
        let fonts = UiFonts::default();
        let look = Look {
            ui: None,
            fonts: &fonts,
            s: 1.0,
            height: 720.0,
            accent: Color::WHITE,
            own_faces: false,
        };
        #[derive(Component)]
        struct Outside;
        let list = ComboList::open(0, 3, 0);
        let entries = ["a".to_string(), "b".into(), "c".into()];
        combo_popup(&mut commands, parent, root, &look, (0.0, 0.0, 100.0, 24.0), &entries, &list, |_| (), Outside);
        commands_queue.apply(app.world_mut());
        let mut q = app.world_mut().query_filtered::<&bevy::ui::FocusPolicy, With<Outside>>();
        assert_eq!(q.single(app.world()).ok(), Some(&bevy::ui::FocusPolicy::Block));
    }


    /// Who a press reached.
    #[derive(Component)]
    struct Clicked(&'static str);

    /// A UI spawned through `Commands` as the menus do, in the scheme's
    /// look at 1:1 (rects are window pixels) on a root filling the
    /// window; then a press at `at`: the `Clicked` nodes it reached,
    /// sorted, and whether a scroll bar's thumb took it.
    fn press(at: Vec2, ui: impl FnOnce(&mut Commands, &Look, Entity)) -> (Vec<&'static str>, bool) {
        let mut app = pointer_test_app();
        let fonts = UiFonts::default();
        let look = Look {
            ui: None,
            fonts: &fonts,
            s: 1.0,
            height: 300.0,
            accent: Color::WHITE,
            own_faces: false,
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world());
        let root = commands
            .spawn(Node {
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            })
            .id();
        ui(&mut commands, &look, root);
        queue.apply(app.world_mut());
        // Laid out first, then the cursor there, then the press.
        app.update();
        app.update();
        let mut script = PointerScript::default();
        script.0.push_back(PointerStep { at, button: None });
        script.0.push_back(PointerStep { at, button: Some(true) });
        app.insert_resource(script);
        app.update();
        app.update();
        let mut q = app.world_mut().query::<(&Clicked, &Interaction)>();
        let mut hit: Vec<_> = q
            .iter(app.world())
            .filter(|(_, i)| **i == Interaction::Pressed)
            .map(|(c, _)| c.0)
            .collect();
        hit.sort();
        let mut q = app.world_mut().query::<(&ScrollThumb, &Interaction)>();
        let thumb = q.iter(app.world()).any(|(_, i)| *i == Interaction::Pressed);
        (hit, thumb)
    }

    /// A button drawn over another clickable takes the click alone: the
    /// one under it (here a list row) never sees the press. Before
    /// `clickable`, `Node` ahead of `Button` left every button `Pass`, and
    /// both were pressed.
    #[test]
    fn a_button_over_another_takes_the_click_alone() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            let row = commands
                .spawn((place(look, 10.0, 10.0, 300.0, 40.0), clickable(), Clicked("row"), ChildOf(root)))
                .id();
            let state = ButtonState::enabled(true);
            button(commands, row, look, (200.0, 5.0, 80.0, 30.0), "OK", state, 0, Clicked("button"));
        };
        assert_eq!(press(Vec2::new(250.0, 30.0), ui).0, ["button"]);
        // Beside the button: the row.
        assert_eq!(press(Vec2::new(50.0, 30.0), ui).0, ["row"]);
    }

    /// A scroll bar laid over a list's right edge: its thumb and arrows
    /// take a press without the row under them seeing it (a drag of the
    /// thumb picked the row too).
    #[test]
    fn a_scroll_bars_thumb_over_a_row_takes_the_click_alone() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            commands.spawn((place(look, 0.0, 0.0, 300.0, 220.0), clickable(), Clicked("row"), ChildOf(root)));
            let bar = scroll_bar(commands, root, look, (283.0, 0.0, 220.0), (100, 10, 0), ("menu", 0), |step| {
                Clicked(if step == 1 { "arrow" } else { "other" })
            });
            commands.entity(bar).insert(ZIndex(1));
        };
        // The thumb: the first rows shown, at the bar's top past its arrow.
        assert_eq!(press(Vec2::new(290.0, 25.0), ui), (vec![], true));
        // The down arrow at its bottom.
        assert_eq!(press(Vec2::new(290.0, 212.0), ui), (vec!["arrow"], false));
        // Off the bar: the row.
        assert_eq!(press(Vec2::new(100.0, 100.0), ui), (vec!["row"], false));
    }

    /// A frame's close box takes the click (not the frame under it); a
    /// frame stops a click on its blank face from reaching the menu row
    /// under the dialog.
    #[test]
    fn a_frame_and_its_close_box_stop_the_click() {
        let ui = |commands: &mut Commands, look: &Look, root: Entity| {
            commands.spawn((place(look, 0.0, 0.0, 400.0, 300.0), clickable(), Clicked("menu row"), ChildOf(root)));
            let parts = frame(commands, root, look, FrameSpec::new("test"), (210.0, 200.0), "Test");
            // `place_frames` puts it on screen; here at the corner.
            commands.entity(parts.frame).insert(Clicked("frame")).entry::<Node>().and_modify(|mut n| {
                n.left = px(0.0);
                n.top = px(0.0);
            });
            commands.entity(parts.close.unwrap()).insert(Clicked("close"));
        };
        assert_eq!(press(Vec2::new(195.0, 14.0), ui).0, ["close"]);
        assert_eq!(press(Vec2::new(100.0, 100.0), ui).0, ["frame"]);
        assert_eq!(press(Vec2::new(300.0, 250.0), ui).0, ["menu row"]);
    }

    /// Anything with an `Interaction` stops the pointer however it was
    /// spawned (`block_clicks`): `Node` first and the `Interaction`
    /// inserted after included; `PassClicks` lets the press through as well.
    #[test]
    fn an_interaction_blocks_unless_it_passes_clicks() {
        for (pass, want) in [(false, vec!["top"]), (true, vec!["top", "under"])] {
            let ui = |commands: &mut Commands, look: &Look, root: Entity| {
                commands.spawn((place(look, 0.0, 0.0, 100.0, 100.0), Clicked("under"), ChildOf(root))).insert(Interaction::default());
                let mut top = commands.spawn((place(look, 0.0, 0.0, 50.0, 50.0), ChildOf(root)));
                top.insert((Interaction::default(), Clicked("top")));
                if pass {
                    top.insert(PassClicks);
                }
            };
            assert_eq!(press(Vec2::new(20.0, 20.0), ui).0, want, "PassClicks {pass}");
        }
    }

    /// `ui_click` and `ui_drag`: the cursor there, the button pressed and
    /// let go as Bevy's input messages, one step a frame.
    #[test]
    fn scripted_pointer_clicks_and_drags() {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<PointerScript>()
            .add_systems(PreUpdate, run_pointer_script.before(bevy::input::InputSystems));
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut()
            .resource_mut::<PointerScript>()
            .drag(Vec2::new(10.0, 20.0), Vec2::new(10.0, 60.0), 4);
        let mut seen = Vec::new();
        for _ in 0..12 {
            app.update();
            let down = app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left);
            let mut q = app.world_mut().query::<&Window>();
            let at = q.single(app.world()).unwrap().cursor_position();
            seen.push((at.map(|p| p.y), down));
        }
        assert_eq!(seen[0], (Some(20.0), false));
        assert_eq!(seen[1], (Some(20.0), true), "pressed");
        assert!(seen[2..8].iter().all(|(_, down)| *down), "held while it moves");
        assert_eq!(seen[7], (Some(60.0), true));
        assert_eq!(seen[8], (Some(60.0), false), "let go at the end");
        assert!(app.world().resource::<PointerScript>().0.is_empty());
    }

    /// A dialog's frame stacks over the game menu's layer and hides what
    /// of the menu is under it: drawn over the main menu's picture (the
    /// part under the frame), or over black in a game; no menu: see-through.
    #[test]
    fn frames_draw_over_the_menu_and_hide_it() {
        let mut app = App::new();
        app.init_resource::<Windows>()
            .init_resource::<FrameBacking>()
            .add_systems(Update, place_frames);
        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        app.world_mut().spawn((window, PrimaryWindow));
        let frame = app
            .world_mut()
            .spawn((
                VguiFrame {
                    id: "options",
                    size: Vec2::new(512.0, 406.0),
                    scale: 1.0,
                    sizeable: None,
                    modal: false,
                },
                Node::default(),
                GlobalZIndex(FRAME_Z),
                BackgroundColor(Color::NONE),
            ))
            .id();
        let picture = Handle::<Image>::default();
        app.world_mut().resource_mut::<FrameBacking>().0 = Some(Backing::Picture(picture.clone(), Vec2::new(2560.0, 1440.0)));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        assert!(e.get::<GlobalZIndex>().unwrap().0 >= FRAME_Z);
        assert_eq!(e.get::<BackgroundColor>().unwrap().0, Color::BLACK, "opaque under the picture");
        let image = e.get::<ImageNode>().expect("the picture under it");
        // Centred: (384, 157) to (896, 563), 1 px in, at twice the size.
        let rect = image.rect.unwrap();
        assert_eq!((rect.min, rect.max), (Vec2::new(770.0, 316.0), Vec2::new(1790.0, 1124.0)));
        // In a game: over the world drawn again, dimmed.
        let world = Handle::<Image>::default();
        app.insert_resource(WorldPicture(Some((world.clone(), Vec2::new(1280.0, 720.0)))));
        app.world_mut().resource_mut::<FrameBacking>().0 = Some(Backing::World(Color::BLACK));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        let image = e.get::<ImageNode>().expect("the world under it");
        let tint = image.color.to_srgba();
        assert_eq!((tint.red, tint.alpha), (WORLD_UNDER_DIALOGS, 1.0), "dimmed, opaque: the menu under it hidden");
        assert_eq!(image.rect.unwrap().min, Vec2::new(385.0, 158.0));
        // Before the world's picture exists: over black.
        app.insert_resource(WorldPicture(None));
        app.update();
        app.update();
        let e = app.world().entity(frame);
        assert!(e.get::<ImageNode>().is_none());
        assert_eq!(e.get::<BackgroundColor>().unwrap().0, Color::BLACK);
        // No menu: see-through as the scheme says.
        app.world_mut().resource_mut::<FrameBacking>().0 = None;
        app.update();
        assert_eq!(app.world().entity(frame).get::<BackgroundColor>().unwrap().0, Color::NONE);
    }

    #[test]
    fn tabs_are_as_wide_as_their_words_and_stay_on_the_sheet() {
        // The options' six tabs at their words' widths (Tahoma-like).
        let words = [52.0, 38.0, 34.0, 33.0, 32.0, 70.0];
        let w = tab_widths(&words, None, Some(496.0));
        assert_eq!(w, [76.0, 62.0, 58.0, 57.0, 56.0, 94.0], "words plus 24, at least 56");
        // Wider than the sheet (the 96 each the options drew before: 576
        // in 496): shrunk so the last ends inside it.
        let w = tab_widths(&words, Some(96.0), Some(496.0));
        assert!(w.iter().sum::<f32>() <= 496.0, "{w:?}");
        assert!(w.iter().all(|w| *w > 80.0));
        assert_eq!(tab_widths(&[200.0], None, None), [224.0], "no sheet: as wide as it needs");
    }

    #[test]
    fn a_frame_opens_centred_then_stays_where_it_was_dragged() {
        let mut w = Windows::default();
        let size = Vec2::new(400.0, 300.0);
        assert_eq!(w.place("options", size, SCREEN), Vec2::new(440.0, 210.0));
        // Dragged by its title bar 100 right, 50 up.
        w.begin("options", None, Vec2::new(500.0, 220.0), 1.0, Vec2::ZERO);
        let p = w.drag_to(Vec2::new(600.0, 170.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(540.0, 160.0));
        w.end_drag();
        // Drawn again (closed and opened): where it was left.
        assert_eq!(w.place("options", size, SCREEN), Vec2::new(540.0, 160.0));
    }

    #[test]
    fn a_dragged_frame_stays_on_screen() {
        let mut w = Windows::default();
        let size = Vec2::new(400.0, 300.0);
        w.place("servers", size, SCREEN);
        w.begin("servers", None, Vec2::new(640.0, 220.0), 1.0, Vec2::ZERO);
        let p = w.drag_to(Vec2::new(-5000.0, 9000.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(0.0, 420.0), "left edge and bottom edge");
        let p = w.drag_to(Vec2::new(9000.0, -9000.0), SCREEN).unwrap();
        assert_eq!(p.pos, Vec2::new(880.0, 0.0));
        // A smaller window later: pulled back on.
        w.end_drag();
        assert_eq!(w.place("servers", size, Vec2::new(800.0, 600.0)), Vec2::new(400.0, 0.0));
        // Bigger than the screen: its top-left corner shows.
        assert_eq!(
            w.place("servers", Vec2::new(900.0, 700.0), Vec2::new(800.0, 600.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn a_clicked_frame_comes_to_the_front() {
        let mut w = Windows::default();
        for id in ["options", "servers", "createserver"] {
            w.place(id, Vec2::splat(100.0), SCREEN);
        }
        assert_eq!(w.front(&["options", "servers"]), Some("servers"));
        w.raise("options");
        assert_eq!(w.front(&["options", "servers", "createserver"]), Some("options"));
        assert!(w.rank("options") > w.rank("createserver"));
        // Dragging raises too.
        w.begin("servers", None, Vec2::ZERO, 1.0, Vec2::ZERO);
        assert_eq!(w.front(&["options", "servers"]), Some("servers"));
        // Not shown yet: in front when it comes.
        assert_eq!(w.rank("advanced"), 3);
    }

    #[test]
    fn a_sizeable_frame_resizes_from_its_edges_down_to_its_minimum() {
        let mut w = Windows::default();
        w.place("servers", Vec2::new(624.0, 384.0), SCREEN);
        let p = w.placed("servers").unwrap();
        let corner = p.pos + p.size - Vec2::splat(2.0);
        let edges = edges_at(p, corner, GRIP, CORNER);
        assert!(edges.right && edges.bottom && !edges.left && !edges.top);
        assert!(
            !edges_at(p, p.pos + p.size / 2.0, GRIP, CORNER).any(),
            "the middle doesn't resize"
        );
        // Scheme scale 2: window pixels are twice scheme pixels.
        w.begin("servers", Some(edges), corner, 2.0, Vec2::new(200.0, 150.0));
        let before = w.resized;
        w.drag_to(corner + Vec2::new(100.0, 40.0), SCREEN);
        assert_eq!(w.size("servers"), Some(Vec2::new(362.0, 212.0)));
        assert!(w.resized > before, "the dialog draws again");
        // Not under the minimum.
        w.drag_to(corner - Vec2::splat(5000.0), SCREEN);
        assert_eq!(w.size("servers"), Some(Vec2::new(200.0, 150.0)));
        // From the left edge: the right edge stays.
        w.end_drag();
        let p = w.placed("servers").unwrap();
        let left = Vec2::new(p.pos.x + 1.0, p.pos.y + p.size.y / 2.0);
        let e = edges_at(p, left, GRIP * 2.0, CORNER * 2.0);
        assert!(e.left && !e.right);
        w.begin("servers", Some(e), left, 2.0, Vec2::new(200.0, 150.0));
        let q = w.drag_to(left - Vec2::new(50.0, 0.0), SCREEN).unwrap();
        assert_eq!(q.pos.x + q.size.x, p.pos.x + p.size.x);
        assert_eq!(q.size.x, p.size.x + 50.0);
    }

    #[test]
    fn a_combo_list_opens_highlights_picks_and_closes() {
        let mut l = ComboList::open(3, 25, 12);
        assert_eq!((l.highlight, l.rows()), (12, COMBO_ROWS));
        assert!(l.first <= 12 && 12 < l.first + COMBO_ROWS, "the selection shows");
        l.hover(14);
        assert_eq!(l.highlight, 14);
        l.hover(99);
        assert_eq!(l.highlight, 14, "past the end: no change");
        assert_eq!(l.key(ComboKey::Down), ComboEvent::Open);
        assert_eq!(l.key(ComboKey::Enter), ComboEvent::Pick(15));
        assert_eq!(l.key(ComboKey::Escape), ComboEvent::Close);
        assert_eq!(l.key(ComboKey::End), ComboEvent::Open);
        assert_eq!((l.highlight, l.first), (24, 15));
        assert_eq!(l.key(ComboKey::Home), ComboEvent::Open);
        assert_eq!((l.highlight, l.first), (0, 0));
        l.key(ComboKey::Up);
        assert_eq!(l.highlight, 0, "no wrapping");
        l.wheel(100);
        assert_eq!(l.first, 15);
        l.wheel(-3);
        assert_eq!(l.first, 12);
        // A short list shows all its entries.
        let s = ComboList::open(0, 3, 0);
        assert_eq!(s.rows(), 3);
        assert_eq!(ComboList::open(0, 0, 0).clone().key(ComboKey::Down), ComboEvent::Close);
    }

    #[test]
    fn a_closed_combo_steps_without_wrapping() {
        assert_eq!(combo_step(0, 4, -1), 0);
        assert_eq!(combo_step(2, 4, 1), 3);
        assert_eq!(combo_step(3, 4, 1), 3);
        assert_eq!(combo_step(0, 0, 1), 0);
    }

    #[test]
    fn a_slider_drag_maps_the_pointer_along_its_track() {
        assert_eq!(slider_fraction(100.0, 100.0, 200.0), 0.0);
        assert_eq!(slider_fraction(200.0, 100.0, 200.0), 0.5);
        assert_eq!(slider_fraction(900.0, 100.0, 200.0), 1.0, "past the end: the end");
        assert_eq!(slider_fraction(0.0, 100.0, 200.0), 0.0);
        assert_eq!(slider_fraction(5.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn text_entry_editing() {
        let mut t = String::from("de_dust");
        let mut c = Caret::end_of(&t);
        c.edit(&mut t, Edit::Insert("2".into()), 64);
        assert_eq!((t.as_str(), c.at), ("de_dust2", 8));
        // Shift+Home selects to the start; typing replaces it.
        c.edit(&mut t, Edit::Home(true), 64);
        assert_eq!(c.selected(&t), "de_dust2");
        c.edit(&mut t, Edit::Insert("cs_".into()), 64);
        assert_eq!((t.as_str(), c), ("cs_", Caret { at: 3, anchor: 3 }));
        // Left with a selection goes to its start; Backspace and Delete.
        t = "abcdef".into();
        c = Caret { at: 4, anchor: 2 };
        assert_eq!(c.selected(&t), "cd");
        c.edit(&mut t, Edit::Left(false), 64);
        assert_eq!(c, Caret { at: 2, anchor: 2 });
        assert!(c.edit(&mut t, Edit::Backspace, 64));
        assert_eq!((t.as_str(), c.at), ("acdef", 1));
        assert!(c.edit(&mut t, Edit::Delete, 64));
        assert_eq!(t, "adef");
        c.edit(&mut t, Edit::Home(false), 64);
        assert!(!c.edit(&mut t, Edit::Backspace, 64), "nothing before the caret");
        c.edit(&mut t, Edit::SelectAll, 64);
        assert_eq!(c.selected(&t), "adef");
        c.edit(&mut t, Edit::Delete, 64);
        assert_eq!(t, "");
        // Control characters dropped, the limit kept, chars not bytes.
        c.edit(&mut t, Edit::Insert("é\u{7}x\nyz".into()), 4);
        assert_eq!((t.as_str(), c.at), ("éxy", 3));
        c.edit(&mut t, Edit::To(1, false), 64);
        c.edit(&mut t, Edit::Right(true), 64);
        assert_eq!(c.selected(&t), "x");
        assert_eq!(c.range(), 1..2);
    }

    #[test]
    fn a_click_lands_on_the_nearest_char_boundary() {
        let offsets = [0.0, 7.0, 14.0, 21.0];
        assert_eq!(char_at(&offsets, 2.0), 0);
        assert_eq!(char_at(&offsets, 4.0), 1);
        assert_eq!(char_at(&offsets, 100.0), 3);
        assert_eq!(char_at(&[], 3.0), 0);
    }

    #[test]
    fn tab_and_shift_tab_walk_the_focus_order() {
        let order = ['a', 'b', 'c'];
        assert_eq!(focus_step(&order, None, 1), Some('a'));
        assert_eq!(focus_step(&order, None, -1), Some('c'));
        assert_eq!(focus_step(&order, Some('c'), 1), Some('a'));
        assert_eq!(focus_step(&order, Some('a'), -1), Some('c'));
        assert_eq!(focus_step(&order, Some('z'), 1), Some('a'));
        assert_eq!(focus_step::<char>(&[], None, 1), None);
    }

