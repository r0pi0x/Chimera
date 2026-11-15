//! The shell (chimera_ui::shell): the bar's racks in their fixed order,
//! unbuilt features disabled, right-click and the breadcrumb pressing Esc,
//! and the avatar's looks.

use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};
use chimera_ui::{
    ChimeraUiPlugin, IconImages, Theme, UiClock, UiUnits,
    avatar::{self, AvatarLook, AvatarMarks, EdgeState, Face, YouFrame},
    controls::{Hover, Ui},
    shell::{self, BackGuard, Bar, Disabled, Left, Member, ShellBack, ShellScreen, ShellTile},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.init_resource::<ButtonInput<MouseButton>>();
    app.init_resource::<Assets<Image>>();
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app
}

fn me() -> Member {
    Member {
        name: "Player".into(),
        face: Face::Look(AvatarLook::new(1, 0)),
        marks: AvatarMarks { you: true, ..default() },
        state: EdgeState::Ready,
        lead: true,
    }
}

fn spawn_bar(app: &mut App, left: Left, open: Option<ShellTile>) -> shell::BarParts {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let party = [me()];
    let world = app.world_mut();
    let mut images = world.remove_resource::<Assets<Image>>().unwrap();
    let mut icons = world.remove_resource::<IconImages>().unwrap();
    let root = world.spawn((Node::default(), ShellScreen)).id();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let parts = {
        let mut c = Commands::new(&mut queue, world);
        let ui = Ui::new(&theme, None, units);
        shell::bar(&ui, &mut c, root, &mut images, &mut icons, &Bar { left, party: &party, party_max: 8, open })
    };
    queue.apply(world);
    world.insert_resource(images);
    world.insert_resource(icons);
    app.update();
    parts
}

/// The Esc messages written since the last call.
fn escapes(app: &mut App) -> Vec<ButtonState> {
    #[derive(Resource, Default)]
    struct Seen(bevy::ecs::message::MessageCursor<KeyboardInput>);
    app.world_mut().init_resource::<Seen>();
    app.world_mut().resource_scope(|w, mut seen: Mut<Seen>| {
        let msgs = w.resource::<Messages<KeyboardInput>>();
        seen.0.read(msgs).filter(|k| k.key_code == KeyCode::Escape).map(|k| k.state).collect()
    })
}

#[test]
fn the_racks_keep_their_order_and_unbuilt_features_are_disabled() {
    let mut app = app();
    let parts = spawn_bar(&mut app, Left::Crumb { parent: "Main menu", title: "Settings" }, Some(ShellTile::Settings));
    let order: Vec<ShellTile> = parts.tiles.iter().map(|(t, _)| *t).collect();
    assert_eq!(order, [ShellTile::Profile, ShellTile::Invite, ShellTile::Friends, ShellTile::Settings, ShellTile::Report]);
    let w = app.world();
    for (t, e) in &parts.tiles {
        let disabled = w.get::<Disabled>(*e).is_some();
        assert_eq!(disabled, matches!(t, ShellTile::Invite | ShellTile::Friends), "{t:?}");
        if disabled {
            assert!(w.get::<Hover>(*e).is_none(), "{t:?}: no hover lift");
            assert!(w.get::<shell::Tip>(*e).is_some_and(|tip| tip.note.as_deref() == Some(shell::COMING_SOON)), "{t:?}: its tooltip says so");
        }
    }
    // Settings is open: a white tile.
    let settings = parts.tile(ShellTile::Settings).unwrap();
    let fg = app.world().resource::<Theme>().tone(chimera_ui::Tone::Fg);
    assert_eq!(app.world().get::<BackgroundColor>(settings).unwrap().0, fg, "the open overlay's tile is white");
    assert!(parts.crumb.is_some() && parts.title.is_some());
    // Your avatar has the YOU frame.
    assert_eq!(app.world_mut().query::<&YouFrame>().iter(app.world()).count(), 1);
}

#[test]
fn right_click_and_the_crumb_press_escape_once() {
    let mut app = app();
    let parts = spawn_bar(&mut app, Left::Crumb { parent: "Join", title: "Lobby" }, None);
    // A right click: Esc pressed this frame, released the next.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    app.update();
    assert_eq!(escapes(&mut app), [ButtonState::Pressed]);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    app.update();
    assert_eq!(escapes(&mut app), [ButtonState::Released]);
    app.update();
    assert!(escapes(&mut app).is_empty(), "held, it goes back once");
    // A click on the breadcrumb.
    let crumb = parts.crumb.unwrap();
    assert!(app.world().get::<ShellBack>(crumb).is_some());
    *app.world_mut().get_mut::<Interaction>(crumb).unwrap() = Interaction::Pressed;
    app.update();
    assert_eq!(escapes(&mut app), [ButtonState::Pressed]);
    app.update();
    app.update();
    assert_eq!(escapes(&mut app), [ButtonState::Released]);
    // Typing (a guard held) or no shell screen: nothing.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
    app.world_mut().resource_mut::<BackGuard>().hold();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    app.update();
    assert!(escapes(&mut app).is_empty(), "not while a field has the keys");
}

#[test]
fn no_back_without_a_shell_screen() {
    let mut app = app();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    app.update();
    assert!(escapes(&mut app).is_empty(), "in a match right-click is the game's");
}

#[test]
fn avatar_looks_round_trip_and_draw_from_the_palette() {
    for b in 0..32u8 {
        let l = AvatarLook::from_byte(b).unwrap();
        assert_eq!(l.byte(), b);
        assert!(chimera_ui::Icons::shipped().get(l.glyph_key()).is_some(), "{} ships", l.glyph_key());
    }
    assert!(AvatarLook::from_byte(AvatarLook::NONE).is_none());
    // The draw covers every glyph and colour.
    let mut glyphs = [false; 4];
    let mut colours = [false; 8];
    for seed in 0..400u64 {
        let l = AvatarLook::random(seed);
        glyphs[l.glyph as usize] = true;
        colours[l.colour as usize] = true;
    }
    assert!(glyphs.iter().all(|g| *g) && colours.iter().all(|c| *c), "{glyphs:?} {colours:?}");
    assert_eq!(AvatarLook::new(0, 0).colour_name(), "Ice");
    // Idle greys both.
    let g = avatar::grey(AvatarLook::new(0, 4).tint()).to_srgba();
    assert!((g.red - g.green).abs() < 1e-6 && (g.green - g.blue).abs() < 1e-6);
    for key in ["gear", "friends", "bug", "plus", "listchev", "mouse_right", "eye_off", "profile"] {
        assert!(chimera_ui::Icons::shipped().get(key).is_some(), "{key} ships");
    }
}
