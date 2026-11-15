//! The loading screen in the Signal look (the kit's design rules 11.6,
//! 13 and 15; docs/plans/active/ugg-ui.md "Signal revision 8", stage 4),
//! for an app whose theme lays its pages out (`[layout.pages]`): over the
//! menu's loading panel while joining a server or loading a map (Start,
//! a join, a map change between matches), unless the pre-match lobby
//! owns the screen.
//!
//! - **The shell**: the breadcrumb ("‹ Join", "‹ Host" for a listen
//!   host's own load) over "Connecting" (or "Loading" between matches);
//!   Back (the breadcrumb, Esc or a right click) asks first.
//! - **The map**: its big card (a gradient stands in for previews) with
//!   its name, its game and the server's name.
//! - **Progress**: the stage's words and its share over the big bar (the
//!   progress component, 26 x 1.6 cq: hollow and filling, striped while
//!   no share is known), then the bytes ("182.4 of 291.0 MB"), the files
//!   ("214 of 340 files", when the asset service told them), the speed
//!   (over the last seconds) and the time left; at the right the stages,
//!   each with its small bar (filled done, hollow running, dim pending);
//!   one sentence in the text face.
//! - **Leave?** (`LoadingSignal::ask`): Esc or a right click asks; Enter
//!   or a click on Leave (the dialog's hero) leaves (the menu's Cancel);
//!   Esc or a right click again, or Stay, stays.
//!
//! `ui_scene load [ask]` stages it (with `+mainmenu` on a tool run).
//!
//! Known shortcuts: the map's preview is a gradient; a map's load has no
//! share (striped); the files show only for the asset service's sets.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_net::net::client::{JoinProgress, JoinStage};
use chimera_ui::{
    IconImages, KitFonts, Role, Theme, Tone, UiStyle, UiUnits,
    controls::{HERO_H, Kind, Progress, State, Ui},
    shell,
};

use crate::main_menu::{MainMenuAction, PartyMenu, Screen};

/// The screen's state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LoadingSignal {
    /// The Leave? dialog is up.
    pub ask: bool,
    /// Esc was this screen's this frame (the menu leaves it).
    pub esc_taken: bool,
    /// The speed's samples: (real s, bytes).
    samples: Vec<(f64, u64)>,
}

impl LoadingSignal {
    /// Bytes a second over the last few seconds' samples.
    pub fn speed(&self) -> Option<f64> {
        let (a, b) = (self.samples.first()?, self.samples.last()?);
        let dt = b.0 - a.0;
        (dt > 0.5 && b.1 >= a.1).then(|| (b.1 - a.1) as f64 / dt)
    }
}

/// The screen's root.
#[derive(Component)]
pub struct LoadingSignalRoot;

/// The Leave? dialog's buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveAsk {
    Stay,
    Leave,
}

/// `ui_scene load [ask]`: a made-up join (de_dust2 on "Back room", the
/// assets 63 % down at 6.1 MB/s), the Leave? dialog up with `ask`.
#[derive(Resource, Clone, Debug)]
pub struct StagedLoad {
    pub join: JoinProgress,
    pub speed: f64,
}

fn stage(w: &mut World, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        w.remove_resource::<StagedLoad>();
        w.resource_mut::<LoadingSignal>().ask = false;
        return Ok(());
    }
    let mut join = JoinProgress { stage: JoinStage::Downloading, map: Some("cs_source:de_dust2".into()), ..default() };
    join.download = Some(chimera_net::net::client::DownloadInfo {
        file: "files".into(),
        from: "asset service".into(),
        done: 182_400_000,
        total: 291_000_000,
        files_done: 214,
        files_total: 340,
    });
    w.insert_resource(StagedLoad { join, speed: 6.1e6 });
    w.resource_mut::<LoadingSignal>().ask = args.first().is_some_and(|a| a == "ask");
    Ok(())
}

/// Whether it shows: themed, loading or joining, the lobby not up.
fn up(world: &World) -> bool {
    if world.contains_resource::<StagedLoad>() {
        return true;
    }
    let themed = crate::signal_pages::themed(world.get_resource::<Theme>());
    let screen = world.get_resource::<PartyMenu>().map(|m| m.screen);
    let lobby = crate::main_menu::owned_by_lobby(world.get_resource::<chimera_client::client::prematch_lobby::LobbyView>());
    themed && matches!(screen, Some(Screen::Loading | Screen::Joining)) && !lobby
}

pub struct LoadingSignalPlugin;

impl Plugin for LoadingSignalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingSignal>()
            .add_systems(Update, (keys, clicks).chain().before(crate::main_menu::MainMenuSystems))
            .add_systems(Update, draw.after(crate::main_menu::MainMenuSystems));
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "load", stage);
    }
}

/// Esc asks (or stays), Enter leaves while asking.
fn keys(world: &mut World) {
    let shown = up(world);
    let mut s = world.resource_mut::<LoadingSignal>();
    s.esc_taken = false;
    if !shown {
        s.ask = false;
        return;
    }
    let Some(keys) = world.get_resource::<ButtonInput<KeyCode>>() else { return };
    let esc = keys.just_pressed(KeyCode::Escape);
    let enter = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]);
    let ask = world.resource::<LoadingSignal>().ask;
    if esc {
        world.resource_mut::<LoadingSignal>().esc_taken = true;
        world.resource_mut::<LoadingSignal>().ask = !ask;
        crate::ui_cues::cue(world, chimera_ui::UiCue::new(if ask { "ui_back" } else { "ui_select" }));
    } else if enter && ask {
        world.resource_mut::<LoadingSignal>().ask = false;
        world.write_message(MainMenuAction::Cancel);
    }
}

fn clicks(
    pressed: Query<(&Interaction, &LeaveAsk), Changed<Interaction>>,
    mut s: ResMut<LoadingSignal>,
    mut out: MessageWriter<MainMenuAction>,
) {
    for (i, a) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        s.ask = false;
        if *a == LeaveAsk::Leave {
            out.write(MainMenuAction::Cancel);
        }
    }
}

/// What the screen was drawn for.
#[derive(Default, PartialEq)]
struct Key {
    up: bool,
    ask: bool,
    stage: Option<JoinStage>,
    bytes: (u64, u64, u64, u64),
    map: String,
    units: (u32, u32),
    secs: u64,
    party: Option<shell::Party>,
}

fn draw(world: &mut World, mut drawn: Local<Key>) {
    let shown = up(world);
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let staged = world.get_resource::<StagedLoad>().cloned();
    let join = match &staged {
        Some(st) => st.join.clone(),
        None => world.get_resource::<JoinProgress>().cloned().unwrap_or_default(),
    };
    let dl = join.download.clone();
    // The speed's samples (the last 4 s).
    {
        let mut s = world.resource_mut::<LoadingSignal>();
        match (&dl, shown) {
            (Some(d), true) if d.total > 0 => {
                s.samples.push((now, d.done));
                s.samples.retain(|(t, _)| now - t <= 4.0);
            }
            _ => s.samples.clear(),
        }
    }
    let Some(units) = world.get_resource::<UiUnits>().copied() else { return };
    let menu = world.resource::<PartyMenu>().clone();
    let map = if staged.is_some() { join.map.clone() } else { menu.loading.clone().or(join.map.clone()) }.unwrap_or_default();
    let key = Key {
        up: shown,
        ask: world.resource::<LoadingSignal>().ask,
        stage: shown.then_some(join.stage),
        bytes: dl.as_ref().map_or((0, 0, 0, 0), |d| (d.done / 100_000, d.total, d.files_done, d.files_total)),
        map: map.clone(),
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        secs: now as u64,
        party: world.get_resource::<shell::Party>().cloned(),
    };
    if *drawn == key {
        return;
    }
    let roots: Vec<Entity> = world.query_filtered::<Entity, With<LoadingSignalRoot>>().iter(world).collect();
    for r in roots {
        world.entity_mut(r).despawn();
    }
    let show = key.up;
    let ask = key.ask;
    *drawn = key;
    if !show {
        return;
    }
    let Some(theme) = world.get_resource::<Theme>().cloned() else { return };
    let kit = world.get_resource::<KitFonts>().cloned();
    let party = world.get_resource::<shell::Party>().cloned().unwrap_or_default();
    let speed = staged.as_ref().map(|s| s.speed).or_else(|| world.resource::<LoadingSignal>().speed());
    let hosting = staged.is_none() && world.get_resource::<chimera_core::core::NetRole>() != Some(&chimera_core::core::NetRole::Client);
    let camera = world.query_filtered::<Entity, With<crate::main_menu::MenuUiCamera>>().iter(world).next();
    let server = join.server.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| if staged.is_some() { "Back room".into() } else { String::new() });
    let mut images = world.remove_resource::<Assets<Image>>();
    let mut icons = world.remove_resource::<IconImages>();
    {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut c = Commands::new(&mut queue, world);
        let ui = Ui::new(&theme, kit.as_ref(), units);
        let root = spawn(
            &mut c,
            &ui,
            &View { stage: join.stage, dl: dl.as_ref(), map: &map, server: &server, hosting, speed, ask, party: &party, between: menu.screen == Screen::Loading && !hosting && join.stage == JoinStage::Idle },
            images.as_mut().zip(icons.as_mut()),
        );
        if let Some(cam) = camera {
            c.entity(root).insert(UiTargetCamera(cam));
        }
        queue.apply(world);
    }
    if let Some(i) = images {
        world.insert_resource(i);
    }
    if let Some(i) = icons {
        world.insert_resource(i);
    }
}

struct View<'a> {
    stage: JoinStage,
    dl: Option<&'a chimera_net::net::client::DownloadInfo>,
    map: &'a str,
    server: &'a str,
    hosting: bool,
    speed: Option<f64>,
    ask: bool,
    party: &'a shell::Party,
    /// A map change between matches (not a join).
    between: bool,
}

/// The stages in order: (words, progress).
fn stages(v: &View) -> Vec<(String, Progress)> {
    use JoinStage as J;
    let rank = |s: J| match s {
        J::Idle | J::Connecting | J::ServerInfo | J::ChangingLevel => 0,
        J::Verifying | J::Downloading => 1,
        J::LoadingMap => 2,
        J::Joined => 3,
    };
    let at = if v.hosting && v.stage == J::Idle { 2 } else { rank(v.stage) };
    let share = v.dl.filter(|d| d.total > 0).map(|d| (d.done as f32 / d.total as f32).clamp(0.0, 1.0));
    let state = |i: usize, running: Progress| match i.cmp(&at) {
        std::cmp::Ordering::Less => Progress::Ready,
        std::cmp::Ordering::Equal => running,
        std::cmp::Ordering::Greater => Progress::Pending,
    };
    let mut out = Vec::new();
    if !v.hosting {
        out.push((if at > 0 { "Connected".to_string() } else { "Connecting".to_string() }, state(0, Progress::Connecting)));
        let words = match v.dl {
            Some(d) if d.total > 0 => {
                let mut w = format!("Downloading assets \u{00b7} {} of {} MB", mb(d.done), mb(d.total));
                if d.files_total > 0 {
                    w += &format!(" \u{00b7} {} of {} files", d.files_done, d.files_total);
                }
                w
            }
            _ => "Downloading assets".to_string(),
        };
        out.push((words, state(1, share.map_or(Progress::Connecting, Progress::Running))));
    }
    out.push(("Loading".to_string(), state(2, Progress::Connecting)));
    out
}

fn mb(b: u64) -> String {
    format!("{:.1}", b as f64 / 1e6)
}

fn spawn(c: &mut Commands, ui: &Ui, v: &View, res: Option<(&mut Assets<Image>, &mut IconImages)>) -> Entity {
    let t = ui.theme;
    let sh = shell::ShellLayout::of(t);
    let root = c
        .spawn((
            LoadingSignalRoot,
            shell::ShellScreen,
            Name::new("loading (Signal)"),
            GlobalZIndex(crate::settings::SETTINGS_Z + 1),
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(t.tone_a(Tone::Ink, 1.0)),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    let frame = ui.boxed(c, root, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let title = if v.between { "Loading" } else { "Connecting" };
    let parent = if v.hosting { "Host" } else { "Join" };
    if let Some((images, icons)) = res {
        shell::bar(ui, c, frame, images, icons, &shell::Bar { left: shell::Left::Crumb { parent, title }, party: &v.party.members, party_max: v.party.max, open: None });
    }
    // The map's card.
    let card_h = 29.2;
    let (a, b) = map_tint(v.map);
    let card = c
        .spawn((
            UiStyle { h: Some(card_h), ..UiStyle::column() }.abs([Some(sh.margin), Some(sh.win_top), Some(sh.margin), None]),
            Node::default(),
            BackgroundGradient::from(LinearGradient::degrees(160.0, vec![ColorStop::percent(a, 0.0), ColorStop::percent(b, 100.0)])),
            ChildOf(frame),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![ColorStop::percent(Color::NONE, 70.0), ColorStop::percent(t.tone_a(Tone::Ink, 0.85), 100.0)])),
        ChildOf(card),
    ));
    let pc = ui.boxed(c, card, UiStyle::row().abs([Some(1.0), Some(1.0), None, None]));
    ui.text(c, pc, Role::Mono, 0.5, Tone::Fg3, "Preview coming");
    let cap = ui.boxed(c, card, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() }.abs([Some(1.8), None, None, Some(1.4)]));
    let name = v.map.rsplit(':').next().unwrap_or(v.map);
    ui.text(c, cap, Role::DisplayBlack, 3.4, Tone::Fg, if name.is_empty() { "-" } else { name });
    let game = v.map.split_once(':').map_or("", |(g, _)| g);
    let mut sub = crate::maps_picker::game_name(game);
    if !v.server.is_empty() {
        sub += &format!(" \u{00b7} {}", v.server);
    }
    ui.text(c, cap, Role::Mono, 0.62, Tone::Fg2, &sub);
    // Progress: the big bar and its numbers at the left, the stages at the
    // right.
    let below = sh.win_top + card_h + 2.6;
    let left = ui.boxed(c, frame, UiStyle { gap: Some(0.7), w: Some(30.2), ..UiStyle::column() }.abs([Some(sh.margin), Some(below), None, None]));
    let all = stages(v);
    let (words, p) = all.iter().find(|(_, p)| !matches!(p, Progress::Ready | Progress::Pending)).cloned().unwrap_or_else(|| all.last().cloned().unwrap());
    let headline = ui.boxed(c, left, UiStyle { gap: Some(0.6), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    let head_words = words.split(" \u{00b7} ").next().unwrap_or(&words).to_string();
    ui.text(c, headline, Role::DisplayRegular, 1.0, Tone::Fg, &head_words);
    if let Progress::Running(f) = p {
        ui.text(c, headline, Role::Mono, 1.0, Tone::Fg, &format!("{:.0}%", f * 100.0));
    }
    ui.progress(c, left, p, "", 30.0, 1.6, 1.0, false);
    let nums = ui.boxed(c, left, UiStyle { gap: Some(1.4), ..UiStyle::row() });
    if let Some(d) = v.dl.filter(|d| d.total > 0) {
        ui.text(c, nums, Role::Mono, 0.66, Tone::Fg, &format!("{} of {} MB", mb(d.done), mb(d.total)));
        if d.files_total > 0 {
            ui.text(c, nums, Role::Mono, 0.66, Tone::Fg2, &format!("{} of {} files", d.files_done, d.files_total));
        }
        if let Some(sp) = v.speed {
            let left_s = if sp > 0.0 { ((d.total.saturating_sub(d.done)) as f64 / sp).ceil() as u64 } else { 0 };
            ui.text(c, nums, Role::Mono, 0.66, Tone::Fg2, &format!("{:.1} MB/s \u{00b7} {} left", sp / 1e6, duration(left_s)));
        }
    }
    let list = ui.boxed(c, frame, UiStyle { gap: Some(0.9), ..UiStyle::column() }.abs([Some(sh.margin + 34.0), Some(below), None, None]));
    for (w, p) in &all {
        let r = ui.boxed(c, list, UiStyle { gap: Some(0.6), ..UiStyle::row() });
        ui.progress(c, r, *p, "", 2.6, 0.8, 0.5, false);
        let tone = if *p == Progress::Pending { Tone::Fg3 } else { Tone::Fg };
        ui.text(c, r, Role::DisplayRegular, 0.66, tone, w);
    }
    let note = ui.boxed(c, frame, UiStyle::row().abs([Some(sh.margin), None, None, Some(sh.win_bottom + 1.6)]));
    ui.sentence(c, note, chimera_ui::controls::SENTENCE, Tone::Fg2, "Assets come from the server once and are cached for next time.");
    if v.ask {
        leave_modal(c, ui, root);
    }
    root
}

/// "45 s", "1:12".
fn duration(secs: u64) -> String {
    if secs >= 60 { format!("{}:{:02}", secs / 60, secs % 60) } else { format!("{secs} s") }
}

/// Leave? (design rules 15): Leave the hero and Enter's; Stay with Esc
/// and the right mouse button.
fn leave_modal(c: &mut Commands, ui: &Ui, root: Entity) {
    let shade = c
        .spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            ZIndex(30),
            ChildOf(root),
        ))
        .id();
    let m = ui.modal(c, shade, 32.0, false);
    ui.text(c, m, Role::DisplayBold, 1.6, Tone::Fg, "Leave?");
    let tx = ui.boxed(c, m, UiStyle { w_pct: Some(100.0), margin: Some([0.8, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
    ui.paragraph(c, tx, Role::Text, chimera_ui::controls::SENTENCE, Tone::Fg2, "You're still loading in. Leaving drops the connection; what has downloaded stays cached for next time.");
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.7), margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let stay = ui.button(c, acts, Kind::Secondary, "Stay", Some("Esc"), 2.4, State::NONE);
    c.entity(stay).insert(LeaveAsk::Stay);
    let leave = ui.button(c, acts, Kind::Primary, "Leave", Some("Enter"), HERO_H, State::focused(true));
    c.entity(leave).insert(LeaveAsk::Leave);
}

/// `map` to a pair of colours, for the preview's stand-in.
fn map_tint(map: &str) -> (Color, Color) {
    let h = map.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32;
    (Color::hsl(hue, 0.25, 0.28), Color::hsl((hue + 40.0) % 360.0, 0.3, 0.12))
}
