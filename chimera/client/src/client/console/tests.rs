use super::*;
    /// The party's saved bug is put back (both volumes 0 together, the
    /// brightness at Combat Arms' default gamma's); a player's own 0
    /// volume with music on, or their own brightness, stays.
    #[test]
    fn repair_options_undoes_the_party_bug_only() {
        use super::{CA_GAMMA_SAVED, repair_options};
        assert_eq!(repair_options(Some(0.0), Some(0.0), Some(1.8032787)), ["volume 0.5", "snd_musicvolume 1", "mat_monitorgamma 2.2"]);
        assert!((CA_GAMMA_SAVED - 1.8032787).abs() < 1e-5);
        assert!(repair_options(Some(0.0), Some(0.6), Some(1.8)).is_empty());
        assert!(repair_options(Some(0.7), Some(0.0), None).is_empty());
        assert!(repair_options(None, None, None).is_empty());
    }


    #[test]
    fn closing_with_the_console_key_captures_the_mouse() {
        use bevy::window::{CursorGrabMode, CursorOptions};
        let mut app = App::new();
        app.init_resource::<ConsoleUi>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, toggle);
        app.world_mut().spawn((ConsoleRoot, Visibility::Hidden));
        let window = app.world_mut().spawn(CursorOptions::default()).id();
        let press = |app: &mut App, key: KeyCode| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(key);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
        };
        let grabbed = |app: &App| app.world().get::<CursorOptions>(window).unwrap().grab_mode != CursorGrabMode::None;
        press(&mut app, KeyCode::Backquote);
        assert!(app.world().resource::<ConsoleUi>().open && !grabbed(&app));
        press(&mut app, KeyCode::Backquote);
        assert!(
            !app.world().resource::<ConsoleUi>().open && grabbed(&app),
            "tilde should recapture"
        );
        press(&mut app, KeyCode::Backquote);
        press(&mut app, KeyCode::Escape);
        assert!(
            !app.world().resource::<ConsoleUi>().open && !grabbed(&app),
            "escape leaves the mouse free"
        );
    }

    use super::*;
    use crate::console::ConsolePlugin;

    /// Exit with changed settings: a play session writes `config.cfg`, every
    /// tool run leaves it alone; the quick exit writes it before it ends the
    /// process. (`CHIMERA_CFG_DIR` is only read by this test
    /// and `cfg_dir`, so the other tests don't care.)
    #[test]
    fn the_config_is_saved_at_exit_but_by_tool_runs_and_before_the_quick_exit() {
        let dir = std::env::temp_dir().join(format!("chimera_toolrun_cfg_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: no other test in this binary reads or writes this variable.
        unsafe { std::env::set_var("CHIMERA_CFG_DIR", &dir) };
        let exits_with = |flags: &str| {
            let args = crate::client::Args::parse_from(flags.split_whitespace().map(String::from)).unwrap();
            let mut app = App::new();
            app.add_plugins(ConsolePlugin)
                .insert_resource(crate::client::ClientArgs(args))
                .add_systems(Last, save_on_exit);
            app.world_mut().resource_mut::<Console>().dirty = true;
            app.world_mut().write_message(AppExit::Success);
            app.update();
            dir.join("config.cfg").exists()
        };
        for flags in ["--screenshot a.png", "--views v.json", "--frames 10", "--bench"] {
            assert!(!exits_with(flags), "{flags} wrote the config");
        }
        assert!(exits_with("+map de_dust2"), "a play session saves the config");
        let _ = std::fs::remove_dir_all(&dir);

        // The quick exit (`chimera_core::shutdown`): the config is written
        // and the shutdown steps run, in their stages' order, before the
        // process's exit call (recorded here), which comes once.
        use chimera_core::shutdown::{ExitCall, ShutdownAppExt, ShutdownPlugin, Stage};
        let args = crate::client::Args::parse_from(["+map".to_string(), "de_dust2".to_string()]).unwrap();
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let at_exit = order.clone();
        let cfg = dir.join("config.cfg");
        let exit = ExitCall(std::sync::Arc::new(move |code| {
            at_exit
                .lock()
                .unwrap()
                .push(format!("exit {code}, config written: {}", cfg.exists()));
        }));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ConsolePlugin, ShutdownPlugin))
            .insert_resource(crate::client::ClientArgs(args))
            .insert_resource(exit)
            .add_systems(Last, save_on_exit);
        for (stage, name) in [(Stage::Settle, "settle"), (Stage::Persist, "outbox"), (Stage::Hide, "hide")] {
            let order = order.clone();
            app.shutdown_flush(stage, name, move |_| order.lock().unwrap().push(name.to_string()));
        }
        app.finish();
        app.update();
        assert!(order.lock().unwrap().is_empty(), "nothing runs before a quit");
        app.world_mut().resource_mut::<Console>().dirty = true;
        app.world_mut().write_message(AppExit::Success);
        app.update();
        app.update();
        assert_eq!(
            *order.lock().unwrap(),
            ["hide", "outbox", "settle", "exit 0, config written: true"],
            "the steps by stage, then the exit, once"
        );
        // `--clean-exit` (the cvar): the steps, no exit call.
        order.lock().unwrap().clear();
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        let (exit, codes) = ExitCall::record();
        app.add_plugins((MinimalPlugins, ConsolePlugin, ShutdownPlugin))
            .insert_resource(exit)
            .insert_resource(chimera_core::shutdown::ExitMode { clean: 1 });
        let o = order.clone();
        app.shutdown_flush(Stage::Persist, "outbox", move |_| o.lock().unwrap().push("outbox".into()));
        app.finish();
        app.world_mut().write_message(AppExit::error());
        app.update();
        assert_eq!(*order.lock().unwrap(), ["outbox"]);
        assert!(codes.lock().unwrap().is_empty(), "a clean exit leaves the process to the runner");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The install is read once per game, games keep separate lists, and
    /// an import (`forget`) reads again; two apps' lists are their own.
    #[test]
    fn map_lists_read_each_games_install_once() {
        let calls = std::cell::Cell::new(0);
        let read = |names: &[&str]| {
            calls.set(calls.get() + 1);
            names.iter().map(|n| n.to_string()).collect::<Vec<_>>()
        };
        let lists = MapLists::default();
        assert_eq!(lists.names_by("a", || read(&["x"])), ["x"]);
        assert_eq!(lists.names_by("a", || read(&["never"])), ["x"]);
        assert_eq!(calls.get(), 1, "asked twice, read once");
        assert_eq!(lists.names_by("b", || read(&["y"])), ["y"]);
        assert_eq!(lists.names_by("a", || read(&["never"])), ["x"], "b didn't drop a's");
        assert_eq!(calls.get(), 2);
        let other = MapLists::default();
        assert_eq!(other.names_by("a", || read(&["z"])), ["z"], "another app's own");
        lists.forget();
        assert_eq!(lists.names_by("a", || read(&["w"])), ["w"]);
        let mut w = World::new();
        w.insert_resource(MapLists::default());
        forget_map_names(&w);
    }

    /// Two apps in one process load a map each: each loading screen sees
    /// its own load's progress (`LoadReporter`), not the other's.
    #[test]
    fn two_apps_keep_their_own_load_progress() {
        use crate::map::loading::LoadReporter;
        fn hooks() -> super::super::AppHooks {
            super::super::AppHooks {
                build: "test",
                load_map: |id, options| {
                    options.progress.report(if id == "g:a" { 0.25 } else { 0.75 }, "stage");
                    Err("stop".into())
                },
                load_map_file: |_, _, _| Err("no".into()),
                map_file_bytes: |_| None,
                unload_greybox: |_| {},
                respawn_greybox: |_| {},
            }
        }
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let (mut a, mut b) = (World::new(), World::new());
        for w in [&mut a, &mut b] {
            w.insert_resource(hooks());
            w.init_resource::<LoadReporter>();
        }
        start_map_load(&mut a, "g:a", None);
        start_map_load(&mut b, "g:b", None);
        for w in [&mut a, &mut b] {
            let task = &mut w.resource_mut::<MapLoad>().task;
            assert!(bevy::tasks::block_on(task).is_err());
        }
        let seen = |w: &World| w.resource::<LoadReporter>().current().map(|p| p.fraction);
        assert_eq!(seen(&a), Some(0.25));
        assert_eq!(seen(&b), Some(0.75));
    }

    #[test]
    fn map_names_with_and_without_their_game() {
        static GAME: GameInfo = GameInfo {
            id: "some_game",
            short_name: "SG",
            content_dir: "sg",
            tick_interval: 0.015,
            open: |_| Err(std::io::ErrorKind::NotFound.into()),
            map_cycle: |_| Vec::new(),
            map_names: |_| Vec::new(),
            map_file: |n| Some(format!("maps/{n}.bsp")),
        steam: None,
            import_map: |_| Err("no".into()),
            default_binds_cfg: "cfg/config_default.cfg",
            default_binds: &[],
            old_default_binds: &[],
            movement_cvars: &[],
            meters_per_unit: 0.0254,
            console_namespaces: &[],
        };
        let target = |name: &str| map_target(Some(&GAME), name);
        let sg = |m: &str| Some((format!("some_game:{m}"), true));
        assert_eq!(target("de_dust2"), sg("de_dust2"));
        // As `+map` and `--map` give it.
        assert_eq!(target("some_game:de_dust2"), sg("de_dust2"));
        assert_eq!(target("SOME_GAME:de_dust2"), sg("de_dust2"));
        assert_eq!(target("greybox"), None);
        assert_eq!(target("mashup:greybox"), None);
        assert_eq!(target("other:arena"), Some(("other:arena".into(), false)));
        // Without a default game, maps need their game's id.
        assert_eq!(map_target(None, "some_game:x"), Some(("some_game:x".into(), false)));
        assert_eq!(map_target(None, "greybox"), None);
    }

    /// Every game's map list (and cycle) is read on a task at startup, so
    /// the first `map` command doesn't open the install on its frame.
    #[test]
    fn map_lists_are_read_ahead_off_the_frame() {
        static GAME: GameInfo = GameInfo {
            id: "ahead_game",
            short_name: "AG",
            content_dir: "ag",
            tick_interval: 0.015,
            open: |_| Err(std::io::ErrorKind::NotFound.into()),
            map_cycle: |_| Vec::new(),
            map_names: |_| Vec::new(),
            map_file: |n| Some(format!("maps/{n}.bsp")),
            steam: None,
            import_map: |_| Err("no".into()),
            default_binds_cfg: "cfg/config_default.cfg",
            default_binds: &[],
            old_default_binds: &[],
            movement_cvars: &[],
            meters_per_unit: 0.0254,
            console_namespaces: &[],
        };
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let mut app = App::new();
        let mut games = Games::default();
        games.register(&GAME);
        app.insert_resource(games)
            .init_resource::<MapLists>()
            .add_systems(Startup, read_map_lists_ahead)
            .add_systems(Update, take_map_lists_ahead);
        for _ in 0..200 {
            app.update();
            if !app.world().contains_resource::<MapListsAhead>() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let lists = app.world().resource::<MapLists>();
        assert!(!app.world().contains_resource::<MapListsAhead>(), "the read finished");
        assert_eq!(lists.names_by("ahead_game", || panic!("read on the frame")), Vec::<String>::new());
        assert!(lists.cycles.lock().unwrap().contains_key("ahead_game"));
    }

    #[test]
    fn fuzzy_ranking() {
        // Prefix beats substring beats scattered letters.
        let mut names = vec!["sv_airaccelerate", "sv_accelerate", "cl_showpos", "sv_autobunnyhopping"];
        names.sort_by_key(|n| score(n, "sv_a").unwrap_or(i32::MAX));
        assert_eq!(names[0], "sv_accelerate");
        assert!(score("sv_enablebunnyhopping", "bunny").unwrap() < score("sv_enablebunnyhopping", "sebh").unwrap());
        assert!(score("sv_enablebunnyhopping", "sebh").is_some(), "letters in order");
        assert!(score("cl_showpos", "xyz").is_none());
    }

    #[test]
    fn selections_cover_text_dragged_either_way() {
        let rows: Vec<String> = ["alpha beta", "gamma", "delta epsilon"].map(String::from).to_vec();
        let sel = ((0, 6), (2, 5));
        assert_eq!(selected_text(&rows, sel), "beta\ngamma\ndelta");
        assert_eq!(selected_text(&rows, (sel.1, sel.0)), "beta\ngamma\ndelta");
        assert_eq!(selected_text(&rows, ((1, 4), (1, 1))), "amm");
        assert_eq!(row_part(sel, 1, 5), Some(0..5));
        assert_eq!(row_part(sel, 3, 5), None);
    }

    /// The console laid out as the game lays it out (Bevy's UI in a small
    /// window): dragging over its output selects the text between the
    /// pointer's two characters, whole rows between, and what is selected
    /// is what Ctrl+C would copy.
    #[test]
    fn dragging_selects_part_of_a_line() {
        use crate::client::widgets::{PointerScript, pointer_test_app};
        let mut app = pointer_test_app();
        app.add_plugins(ConsolePlugin)
            .init_resource::<crate::client::fonts::UiFonts>()
            .init_resource::<ConsoleUi>()
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, (draw_console, select_text).chain());
        app.world_mut().resource_mut::<ConsoleUi>().open = true;
        for line in ["first line of output", "second line of output", "third"] {
            app.world_mut().resource_mut::<Console>().info(line);
        }
        for _ in 0..4 {
            app.update();
        }
        let rows = |app: &mut App| {
            let mut q = app.world_mut().query::<(&ConsoleRow, &ComputedNode, &UiGlobalTransform)>();
            let mut v: Vec<_> = q
                .iter(app.world())
                .map(|(r, n, t)| (r.0, t.translation - n.size() / 2.0, n.size()))
                .collect();
            v.sort_by_key(|r| r.0);
            v
        };
        let r = rows(&mut app);
        assert!(r.len() >= 3, "rows laid out: {r:?}");
        let (second, third) = (r[r.len() - 2], r[r.len() - 1]);
        let y2 = second.1.y + second.2.y / 2.0;
        let y3 = third.1.y + third.2.y / 2.0;
        // From just inside the second row's left edge to well right of the
        // third row's text: the second row whole, the third whole.
        let drag = |app: &mut App, from: Vec2, to: Vec2| {
            app.world_mut().resource_mut::<PointerScript>().drag(from, to, 3);
            for _ in 0..14 {
                app.update();
            }
        };
        drag(&mut app, Vec2::new(second.1.x + 1.0, y2), Vec2::new(390.0, y3));
        let ui = app.world().resource::<ConsoleUi>();
        let sel = ui.selection.expect("a selection");
        assert_eq!(selected_text(&ui.shown, sel), "second line of output\nthird");
        // Inside one line: a part of it, neither end of the row.
        let w = second.2.x.min(300.0);
        drag(&mut app, Vec2::new(second.1.x + w * 0.1, y2), Vec2::new(second.1.x + w * 0.3, y2));
        let ui = app.world().resource::<ConsoleUi>();
        let sel = ui.selection.expect("a selection");
        let text = selected_text(&ui.shown, sel);
        assert!(!text.is_empty() && text.len() < "second line of output".len(), "part of the line: {text:?}");
        assert!("second line of output".contains(&text), "{text:?}");
        // The selected span is drawn as its own span of the row's text.
        let mut spans = app.world_mut().query::<&bevy::text::TextBackgroundColor>();
        assert_eq!(spans.iter(app.world()).count(), 1);
        // A press elsewhere clears it.
        app.world_mut().resource_mut::<PointerScript>().click(Vec2::new(5.0, 290.0));
        for _ in 0..8 {
            app.update();
        }
        assert!(app.world().resource::<ConsoleUi>().selection.is_none());
        assert_eq!(spans.iter(app.world()).count(), 0);
    }

    #[test]
    fn completion_candidates() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin);
        app.init_resource::<HeldActions>();
        client_commands(&mut app);
        app.console_cvar(
            "sv_enablebunnyhopping",
            "bhop",
            "0",
            |_| Some("0".into()),
            |_, _| Ok(()),
        );
        let w = app.world_mut();
        let (head, list) = candidates(w, "sv_enab");
        assert_eq!(head, "");
        assert_eq!(list[0].0, "sv_enablebunnyhopping");
        // A boolean cvar offers 0 and 1.
        let (head, list) = candidates(w, "sv_enablebunnyhopping ");
        assert_eq!(head, "sv_enablebunnyhopping ");
        assert!(list.iter().any(|(v, _)| v == "1"));
        // bind: key names, then actions.
        let (_, keys) = candidates(w, "bind mwh");
        assert!(keys.iter().any(|(k, _)| k == "mwheeldown"));
        let (_, actions) = candidates(w, "bind space +ju");
        assert_eq!(actions[0].0, "+jump");
        // Commands registered by the client are names too.
        let (_, list) = candidates(w, "noc");
        assert_eq!(list[0].0, "noclip");
    }

    /// A console with its editing system, open, and keys typed into it.
    /// config.cfg is shared by the apps: a bind holding another app's
    /// command doesn't complain on every press (the HL2:DM playtest's
    /// `choose_1`, Combat Arms'); what this app has still runs.
    #[test]
    fn a_key_press_leaves_out_commands_this_app_has_not() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, run_binds);
        {
            let mut c = app.world_mut().resource_mut::<Console>();
            c.binds.insert("1".into(), "slot1; choose_1".into());
            c.binds.insert("2".into(), "choose_2; echo still here".into());
        }
        for key in [KeyCode::Digit1, KeyCode::Digit2] {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
            app.update();
        }
        let text: Vec<String> = app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
        assert!(!text.iter().any(|l| l.contains("Unknown command")), "{text:?}");
        assert!(text.iter().any(|l| l.contains("still here")), "{text:?}");
    }

    /// While the pre-match lobby shows, F3 is its key (the host's
    /// `lobby_start`) in place of its bind; after, the bind again.
    #[test]
    fn f3_is_the_lobbys_key_while_it_shows() {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<MouseWheel>()
            .add_systems(Update, run_binds);
        app.console_command("lobby_start", "test", |_, _| Ok(Some("lobby key".into())))
            .console_command("drawcollision_test", "test", |_, _| Ok(Some("bind ran".into())));
        app.world_mut().resource_mut::<Console>().binds.insert("f3".into(), "drawcollision_test".into());
        app.insert_resource(super::super::prematch_lobby::LobbyView {
            shown: true,
            hosting: true,
            ..default()
        });
        let press = |app: &mut App| {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F3);
            app.update();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::F3);
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
            app.update();
            let text: Vec<String> = app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
            app.world_mut().resource_mut::<Console>().output.clear();
            text
        };
        let text = press(&mut app);
        assert!(text.iter().any(|l| l.contains("lobby key")) && !text.iter().any(|l| l.contains("bind ran")), "{text:?}");
        app.insert_resource(super::super::prematch_lobby::LobbyView::default());
        let text = press(&mut app);
        assert!(text.iter().any(|l| l.contains("bind ran")) && !text.iter().any(|l| l.contains("lobby key")), "{text:?}");
    }

    fn typing_app() -> App {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin)
            .init_resource::<HeldActions>()
            .init_resource::<ConsoleUi>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .init_resource::<Overlays>()
            .add_systems(Update, edit);
        client_commands(&mut app);
        overlay_cvars(&mut app);
        app.console_cvar("sv_gravity", "Gravity.", "800", |_| Some("800".into()), |_, _| Ok(()));
        app.world_mut().resource_mut::<Console>().set_range("sv_gravity", 0.0, 2000.0);
        app.world_mut().resource_mut::<ConsoleUi>().open = true;
        app
    }

    fn press(app: &mut App, key_code: KeyCode, logical_key: Key) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            if c == ' ' {
                press(app, KeyCode::Space, Key::Space);
            } else {
                press(app, KeyCode::KeyQ, Key::Character(c.to_string().into()));
            }
        }
    }

    fn input(app: &App) -> String {
        app.world().resource::<ConsoleUi>().input.clone()
    }

    #[test]
    fn enter_on_a_picked_suggestion_takes_it_and_leaves_room_for_arguments() {
        let mut app = typing_app();
        type_text(&mut app, "sv_grav");
        let (_, list) = suggestions(app.world_mut()).expect("a list shows");
        assert_eq!(list[0].0, "sv_gravity");
        // Down picks the first, Up from there wraps to the last.
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        assert_eq!(app.world().resource::<ConsoleUi>().highlight, Some(0));
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(app.world().resource::<ConsoleUi>().highlight, Some(list.len() - 1));
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        let before = app.world().resource::<Console>().output.len();
        press(&mut app, KeyCode::Enter, Key::Enter);
        assert_eq!(input(&app), "sv_gravity ", "taken, with a space for the value");
        assert_eq!(app.world().resource::<Console>().output.len(), before, "nothing ran");
        // Now the help is for its value, and the list offers values.
        let typed = input(&app);
        let help = arg_help(app.world_mut(), &typed).unwrap();
        assert_eq!((help.name.as_str(), help.current), ("sv_gravity", Some(0)));
        assert!(help.detail.contains("= \"800\"") && help.detail.contains("range 0 to 2000"), "{}", help.detail);
        // Esc hides the list; Enter then runs the line.
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert!(suggestions(app.world_mut()).is_none());
        press(&mut app, KeyCode::Enter, Key::Enter);
        let out = &app.world().resource::<Console>().output;
        assert!(out.iter().any(|l| l.text == "] sv_gravity "), "{:?}", out.last());
        assert_eq!(input(&app), "");
    }

    #[test]
    fn tab_takes_the_pick_and_up_browses_history_without_a_list() {
        let mut app = typing_app();
        app.world_mut().resource_mut::<ConsoleUi>().history = vec!["noclip".into(), "god".into()];
        // Nothing typed: no list, Up is history.
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(input(&app), "god");
        press(&mut app, KeyCode::ArrowUp, Key::ArrowUp);
        assert_eq!(input(&app), "noclip");
        // Typing leaves the history; the list is back and Tab takes a pick.
        type_text(&mut app, "; setp");
        press(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        press(&mut app, KeyCode::Tab, Key::Tab);
        assert_eq!(input(&app), "noclip; setpos ");
        let help = arg_help(app.world_mut(), "noclip; setpos 10 ").unwrap();
        assert_eq!(help.args, ["<x>", "<y>", "<z>"]);
        assert_eq!(help.current, Some(1));
        // Ctrl+Backspace deletes a word.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        press(&mut app, KeyCode::Backspace, Key::Backspace);
        assert_eq!(input(&app), "noclip; ");
    }

    #[test]
    fn highlight_steps_and_window() {
        assert_eq!(step_highlight(None, 5, false), Some(0));
        assert_eq!(step_highlight(None, 5, true), Some(4));
        assert_eq!(step_highlight(Some(4), 5, false), Some(0));
        assert_eq!(step_highlight(Some(0), 5, true), Some(4));
        assert_eq!(step_highlight(None, 0, false), None);
        assert_eq!(suggestion_window(Some(3), 20), 0);
        assert_eq!(suggestion_window(Some(SUGGESTIONS), 20), 1);
        assert_eq!(suggestion_window(Some(19), 20), 20 - SUGGESTIONS);
        assert_eq!(accept("bind f ", "+jump"), "bind f +jump ");
        assert_eq!(accept("", "say hi"), "\"say hi\" ");
    }

    #[test]
    fn argument_completion_follows_the_argument() {
        let mut app = typing_app();
        let w = app.world_mut();
        let (_, teams) = candidates(w, "jointeam ");
        assert_eq!(teams.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["1", "2", "3"]);
        let (_, hits) = candidates(w, "chimera_hurtme he");
        assert_eq!(hits[0].0, "head");
        // After the hitgroup, no more hitgroups.
        let (_, after) = candidates(w, "chimera_hurtme head ");
        assert!(after.iter().all(|a| a.0 != "chest"));
        let (head, tabs) = candidates(w, "echo x; debugui pe");
        assert_eq!((head.as_str(), tabs[0].0.as_str()), ("echo x; debugui ", "perf"));
        // A cvar with a small whole range offers each value.
        let (_, values) = candidates(w, "con_timestamps ");
        assert!(values.iter().any(|v| v.0 == "1"));
    }

    #[test]
    fn the_console_fits_the_window() {
        assert!(lines_for_height(1080.0) > lines_for_height(720.0));
        assert!(lines_for_height(720.0) >= 15, "{}", lines_for_height(720.0));
        assert_eq!(lines_for_height(10.0), 4);
    }

    #[test]
    fn reverse_search_and_tokens() {
        let h = vec!["sv_gravity 800".to_string(), "noclip".into(), "sv_gravity 400".into()];
        assert_eq!(search(&h, "grav", 0).as_deref(), Some("sv_gravity 400"));
        assert_eq!(search(&h, "grav", 1).as_deref(), Some("sv_gravity 800"));
        assert_eq!(search(&h, "grav", 2), None);
        assert_eq!(
            split_last("bind space +j"),
            ("bind space ".to_string(), "+j".to_string())
        );
    }
