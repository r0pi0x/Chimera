//! The loading screen in the Signal look (the kit's design rules 11.6,
//! 13 and 15; docs/plans/active/ugg-ui.md "Signal revision 8", stage 4),
//! for an app whose theme lays its pages out (`[layout.pages]`): over the
//! menu's loading panel while joining a server or loading a map (Start,
//! a join, a map change between matches), unless the pre-match lobby
//! owns the screen.
//!
//! - **The shell**: the breadcrumb ("‹ Join", "‹ Host" for a listen
//!   host's own load) over "Connecting" (or "Loading" between matches),
//!   the kit's waiting mark (`Ui::busy`) in the card's corner;
//!   Back (the breadcrumb, Esc or a right click) asks first.
//! - **The map**: its big card (a gradient stands in for previews) with
//!   its name, its game and the server's name.
//! - **Progress**: the stage's words and its share over the big bar (the
//!   progress component, 26 x 1.6 cq: hollow and filling, striped while
//!   no share is known), then the bytes ("182.4 of 291.0 MB"), the files
//!   ("214 of 340 files", when the asset service told them), the speed
//!   (over the last seconds) and the time left; at the right the stages,
//!   each with its small bar (filled done, hollow running, dim pending),
//!   shown only when there is more than one: connecting (a join), then
//!   the load's own from the work done (`chimera_net::net::load_stages`: the
//!   download when there is one, the map's load, building models, physics
//!   and navigation); one sentence in the text face.
//! - **Leave?** (`LoadingSignal::ask`): Esc or a right click asks; Enter
//!   or a click on Leave (the dialog's hero) leaves (the menu's Cancel);
//!   Esc or a right click again, or Stay, stays.
//!
//! The map's name is the map's id without its game; an arena slice's
//! (docs/plans/active/arena-slices.md) once the slice is planned for the
//! map coming in (`ArenaPlan`: the host's at its map change, a client's
//! with the match) is its label (`vote::map_label`: "Blood Gulch: West
//! Base").
//!
//! `ui_scene load [ask|slice]` stages it (with `+mainmenu` on a tool run;
//! `slice`: Blood Gulch's first slice loading).
//!
//! Known shortcuts: the map's preview is a gradient; a stage is striped
//! until its first count (a level change before its load starts); the
//! files show only for the asset service's sets.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_net::net::client::{JoinProgress, JoinStage};
use chimera_ui::{
    IconImages, KitFonts, Role, Theme, Tone, UiStyle, UiUnits,
    controls::{Kind, Progress, State, Ui},
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
    /// The arena slice staged as loading (`ui_scene load slice`).
    pub slice: Option<String>,
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
    // `slice`: Blood Gulch's first registered slice loading.
    let slice = args.first().filter(|a| *a == "slice").and_then(|_| {
        let all = w.get_resource::<chimera_sim::minigame::arena::ArenaSlices>()?;
        all.0.iter().find(|d| d.slice.map == "halo_ce:bloodgulch").map(|d| d.slice.key())
    });
    if slice.is_some() {
        join.map = Some("halo_ce:bloodgulch".into());
    }
    w.insert_resource(StagedLoad { join, speed: 6.1e6, slice });
    w.resource_mut::<LoadingSignal>().ask = args.first().is_some_and(|a| a == "ask");
    Ok(())
}

/// The label of an arena slice planned for `map` (the staged one when
/// staged), `vote::map_label`'s; empty for a whole map.
fn shown_label(world: &World, map: &str, staged: Option<&StagedLoad>) -> String {
    use chimera_sim::minigame::arena::{ArenaPlan, ArenaSlices};
    let key = match staged {
        Some(st) => st.slice.clone(),
        None => world.get_resource::<ArenaPlan>().and_then(|p| p.0.as_ref()).filter(|s| s.map.eq_ignore_ascii_case(map)).map(|s| s.key()),
    };
    key.map(|k| chimera_sim::minigame::vote::map_label(world.get_resource::<ArenaSlices>(), &k)).unwrap_or_default()
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
    label: String,
    units: (u32, u32),
    secs: u64,
    /// The load's stages, each one's share in 2% steps (redrawn as they move).
    load: Vec<(u8, u8)>,
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
    let label = shown_label(world, &map, staged.as_ref());
    // The load's stages (`chimera_net::net::load_stages`), the staged join's
    // when staged.
    let load = match &staged {
        Some(st) => chimera_net::net::load_stages::stages_of(&chimera_net::net::load_stages::Inputs { join: Some(&st.join), ..default() }),
        None => chimera_net::net::load_stages::stages(world),
    };
    let key = Key {
        up: shown,
        ask: world.resource::<LoadingSignal>().ask,
        stage: shown.then_some(join.stage),
        bytes: dl.as_ref().map_or((0, 0, 0, 0), |d| (d.done / 100_000, d.total, d.files_done, d.files_total)),
        map: map.clone(),
        label: label.clone(),
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        secs: now as u64,
        party: world.get_resource::<shell::Party>().cloned(),
        load: load.iter().map(|s| (s.step.index(), stage_key(s.state))).collect(),
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
            &View { stage: join.stage, load: &load, dl: dl.as_ref(), map: &map, label: &label, server: &server, hosting, speed, ask, party: &party, between: menu.screen == Screen::Loading && !hosting && join.stage == JoinStage::Idle },
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
    /// The load's stages (`chimera_net::net::load_stages`).
    load: &'a [chimera_net::net::load_stages::Stage],
    dl: Option<&'a chimera_net::net::client::DownloadInfo>,
    map: &'a str,
    /// The map's name as shown when not its id's (`shown_label`).
    label: &'a str,
    server: &'a str,
    hosting: bool,
    speed: Option<f64>,
    ask: bool,
    party: &'a shell::Party,
    /// A map change between matches (not a join).
    between: bool,
}

/// A stage's state for the redraw key: 0 pending, 1 striped, 2 + its
/// share in 2% steps, 255 done.
fn stage_key(s: chimera_net::net::load_stages::StageState) -> u8 {
    use chimera_net::net::load_stages::StageState as S;
    match s {
        S::Pending => 0,
        S::Running(None) => 1,
        S::Running(Some(f)) => 2 + (f.clamp(0.0, 1.0) * 50.0) as u8,
        S::Done => 255,
    }
}

/// The stages in order: (words, progress): connecting (a join), then the
/// load's own (`chimera_net::net::load_stages`: the download when there is one,
/// the map, the models, the physics), each from its work.
fn stages(v: &View) -> Vec<(String, Progress)> {
    use JoinStage as J;
    use chimera_net::net::load_stages::{LoadStep, StageState};
    let connected = !matches!(v.stage, J::Idle | J::Connecting | J::ServerInfo) || !v.load.is_empty() && v.stage == J::Idle;
    let mut out = Vec::new();
    if !v.hosting {
        out.push(if connected { ("Connected".to_string(), Progress::Ready) } else { ("Connecting".to_string(), Progress::Connecting) });
    }
    let download_words = |bytes: Option<(u64, u64)>, files: Option<(u64, u64)>| match bytes.or(v.dl.map(|d| (d.done, d.total))) {
        Some((done, total)) if total > 0 => {
            let mut w = format!("Downloading assets \u{00b7} {} of {} MB", mb(done), mb(total));
            let files = files.or(v.dl.filter(|d| d.files_total > 0).map(|d| (d.files_done, d.files_total)));
            if let Some((fd, ft)) = files {
                w += &format!(" \u{00b7} {fd} of {ft} files");
            }
            w
        }
        _ => "Downloading assets".to_string(),
    };
    if v.load.is_empty() {
        // Nothing under way yet (connecting): what comes, pending.
        if !v.hosting {
            out.push((download_words(None, None), Progress::Pending));
        }
        for step in [LoadStep::Map, LoadStep::Models, LoadStep::Physics] {
            out.push((step.words().to_string(), if v.hosting { Progress::Connecting } else { Progress::Pending }));
            if v.hosting {
                break;
            }
        }
        return out;
    }
    for s in v.load {
        let words = match s.step {
            LoadStep::Download => download_words(s.bytes, s.files),
            step => step.words().to_string(),
        };
        let p = match s.state {
            StageState::Pending => Progress::Pending,
            StageState::Done => Progress::Ready,
            StageState::Running(Some(f)) => Progress::Running(f),
            StageState::Running(None) => Progress::Connecting,
        };
        out.push((words, p));
    }
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
        shell::bar(ui, c, frame, images, icons, &shell::Bar { left: shell::Left::Crumb { parent, title }, party: &v.party.members, party_max: v.party.max, invite: v.party.invite, open: None });
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
    // The kit's waiting mark in the card's corner while it loads.
    let spin = ui.boxed(c, card, UiStyle::row().abs([None, Some(1.2), Some(1.4), None]));
    ui.busy(c, spin, 8, 0.45);
    c.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![ColorStop::percent(Color::NONE, 70.0), ColorStop::percent(t.tone_a(Tone::Ink, 0.85), 100.0)])),
        ChildOf(card),
    ));
    let pc = ui.boxed(c, card, UiStyle::row().abs([Some(1.0), Some(1.0), None, None]));
    ui.text(c, pc, Role::Mono, 0.5, Tone::Fg3, "Preview coming");
    let cap = ui.boxed(c, card, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() }.abs([Some(1.8), None, None, Some(1.4)]));
    let name = if v.label.is_empty() { v.map.rsplit(':').next().unwrap_or(v.map) } else { v.label };
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
    // The stages at the right, each with its small bar: only with more
    // than one (one stage is the main bar alone).
    let list = ui.boxed(c, frame, UiStyle { gap: Some(0.9), ..UiStyle::column() }.abs([Some(sh.margin + 34.0), Some(below), None, None]));
    for (w, p) in all.iter().filter(|_| all.len() > 1) {
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
    let stay = ui.button(c, acts, Kind::Secondary, "Stay", Some("Esc"), chimera_ui::controls::Size::Regular, State::NONE);
    c.entity(stay).insert(LeaveAsk::Stay);
    let leave = ui.button(c, acts, Kind::Primary, "Leave", Some("Enter"), chimera_ui::controls::Size::Regular, State::focused(true));
    c.entity(leave).insert(LeaveAsk::Leave);
}

/// `map` to a pair of colours, for the preview's stand-in.
fn map_tint(map: &str) -> (Color, Color) {
    let h = map.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32;
    (Color::hsl(hue, 0.25, 0.28), Color::hsl((hue + 40.0) % 360.0, 0.3, 0.12))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_sim::minigame::arena::{self, ArenaPlan, ArenaSlices};

    #[test]
    fn a_slice_loading_shows_its_label() {
        let text = "[[slice]]\nmap = \"bloodgulch\"\nid = \"west_base\"\nname = \"West Base\"\nmap_name = \"Blood Gulch\"\n\
                    points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]\nseed = [5.0, 1.0, 5.0]\n";
        let defs = arena::parse("halo_ce", text).unwrap();
        let mut world = World::new();
        assert_eq!(shown_label(&world, "halo_ce:bloodgulch", None), "", "no plan: the map's id");
        world.insert_resource(ArenaPlan(Some(defs[0].slice.clone())));
        assert_eq!(shown_label(&world, "halo_ce:bloodgulch", None), "Bloodgulch: West Base", "no data: the id's form");
        world.insert_resource(ArenaSlices(defs));
        assert_eq!(shown_label(&world, "halo_ce:bloodgulch", None), "Blood Gulch: West Base");
        assert_eq!(shown_label(&world, "halo_ce:chillout", None), "", "the plan is another map's");
    }
}
