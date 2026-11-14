//! Settings in the Signal look (feature `present`; a theme with
//! `[layout.settings]`, the Ultimate Gun Game's; docs/plans/active/
//! ugg-ui.md M7, the components page's frame 02): one page for the main
//! menu's Options, the Escape menu's Options and the Controls page (F6),
//! and the Escape menu's Host settings in the same window.
//!
//! - **When**: the main menu's Options page (`PartyMenu::page`) over the
//!   menu's ambient (`AmbientMaterial::MENU`), or the Escape menu's
//!   Options or Host settings page over the match (a flat 86 % shade):
//!   `Context`, from those menus' state, so their Esc, Back and actions
//!   keep working. Without the Signal theme those menus draw their own
//!   pages as before.
//! - **Pages** (sub-tabs, Q/E): data/options.toml's tabs, its Mouse tab
//!   becoming **Controls** with the Controls page's key binds under it (a
//!   segmented switch Shared / each game, `controls::GAMES`, each game's
//!   gun view FOV too); Host settings has one page, `host::settings()`,
//!   each row noting "now" or "next match". Only settings that exist.
//! - **Rows** by the setting's pick: a range is a slider (click or drag a
//!   place, ←→ a step), two choices Off/On a toggle, `widget =
//!   "dropdown"` a dropdown, up to five choices segmented, more a stepper;
//!   a bind a key field (Enter or click, then a key, a mouse button or
//!   the wheel; Esc cancels; Delete clears). ↑↓ move the focus (the
//!   pointer moves it too), the focused row's help in the pane at the
//!   right: what it does (the TOML `help`, else the cvar's own), its
//!   console name, its default and its range.
//! - **Apply / Revert** (the bar, Ctrl+S / Ctrl+Z): changes apply at once
//!   (as today; the bar wraps them): the bar counts the settings changed
//!   since the last Apply, Revert sets them back, Apply writes the config;
//!   leaving the page applies (writes the config) as before. A
//!   display-mode change (`chimera_fullscreen`) asks to be kept and goes
//!   back by itself after `layout.settings.keep_secs` (15 s). Binds write
//!   at once and are saved on leaving.
//! - **Motion**: a page change wipes the rows in on the 12° slant
//!   (`controls::Ui::wipe_cover`); screens change through the logo pass
//!   as before.
//! - **Sounds** (`ui_cues`): hover, select, back, a refused press and the
//!   slider's step.
//!
//! `ui_scene settings [tab]` and `ui_scene esc` stage them for
//! screenshots (`chimera_client::client::ui_scene`'s app hooks).
//!
//! Known shortcuts: the Apply bar wraps the immediate writes (no pending
//! values held back until Apply); the dropdown's list has no scroll (the
//! options' lists are short); the help pane has no previews (FOV fan,
//! crosshair); a key already bound elsewhere isn't flagged.

use std::collections::BTreeMap;

use bevy::{
    input::{ButtonInput, mouse::AccumulatedMouseScroll},
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, RelativeCursorPosition, ScrollPosition},
};
use chimera_client::client::{binds, ui_sound::UiCue};
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_ui::{
    IconImages, KitFonts, Role, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{self, Kind, State, Ui},
};

use crate::host::{self, Pick, Setting};

/// The layout table.
const L: &str = "settings";

/// The options tab that becomes Controls (with the binds).
pub const BINDS_TAB: &str = "Mouse";

/// The Signal look draws Settings (a theme with `[layout.settings]`).
pub fn signal(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

/// Where Settings shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// The main menu's Options, over the menu's ambient.
    Menu,
    /// The Escape menu's Options, over the match.
    Game,
    /// The Escape menu's Host settings.
    Host,
}

/// A display-mode change waiting to be kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Keep {
    pub cvar: String,
    pub old: String,
    /// On the UI clock.
    pub until: f64,
}

/// The screen's state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Settings {
    /// Where it shows (None: shut).
    pub ctx: Option<Context>,
    pub tab: usize,
    /// The focused row (an index into `rows`).
    pub focus: usize,
    /// The bind row waiting for a key.
    pub capture: Option<usize>,
    /// The binds shown: 0 shared, n `controls::GAMES[n - 1]`.
    pub bind_game: usize,
    /// An open dropdown: its row and the option the keys are on.
    pub open_list: Option<(usize, usize)>,
    /// Each setting changed since the last Apply, and its value before.
    pub baseline: BTreeMap<String, String>,
    /// A bind changed (saved on leaving).
    pub binds_dirty: bool,
    pub keep: Option<Keep>,
    /// When the page last changed (the wipe), on the UI clock.
    pub page_since: f64,
    /// Esc was this screen's this frame (a capture, a list, the keep):
    /// the menus leave it.
    pub esc_taken: bool,
    /// The shown settings' values as the cvars hold them.
    pub values: BTreeMap<String, String>,
    /// The rows' scroll, cq.
    pub scroll: f32,
    /// A page asked for by name (`ui_scene settings <tab>`) at opening.
    pub want_tab: Option<String>,
    /// A staged scene's page, opened once the menus are up.
    pub stage: Option<String>,
}

/// One row of a page.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Section(String),
    /// A cvar; `note` the host's "now" or "next match".
    Setting { s: Setting, note: Option<&'static str> },
    /// The binds' game switch.
    Games,
    Bind { ctab: usize, label: &'static str, command: &'static str },
}

impl Row {
    pub fn focusable(&self) -> bool {
        !matches!(self, Row::Section(_))
    }

    /// Height in cq.
    fn height(&self, t: &Theme) -> f32 {
        match self {
            Row::Section(_) => 2.4,
            Row::Bind { .. } => t.layout(L, "bind_h"),
            _ => t.layout(L, "row_h"),
        }
    }
}

/// How a setting is changed on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Widget {
    Slider { min: f32, max: f32, step: f32 },
    Toggle,
    Segmented,
    Dropdown,
    Stepper,
}

/// A setting's widget (module doc).
pub fn widget(s: &Setting) -> Widget {
    match &s.pick {
        Pick::Range { min, max, step } => Widget::Slider { min: *min, max: *max, step: *step },
        Pick::Choices(c) => {
            let labels: Vec<String> = c.iter().map(|(_, l)| l.to_ascii_lowercase()).collect();
            if s.widget.as_deref() == Some("dropdown") {
                Widget::Dropdown
            } else if labels == ["off", "on"] {
                Widget::Toggle
            } else if c.len() <= 5 {
                Widget::Segmented
            } else {
                Widget::Stepper
            }
        }
    }
}

/// The pages' names.
pub fn pages(ctx: Context, tabs: &[(String, Vec<Setting>)]) -> Vec<String> {
    match ctx {
        Context::Host => vec!["Host settings".into()],
        _ => tabs.iter().map(|(n, _)| if n == BINDS_TAB { "Controls".to_string() } else { n.clone() }).collect(),
    }
}

/// A page's rows.
pub fn rows(ctx: Context, tabs: &[(String, Vec<Setting>)], tab: usize, bind_game: usize) -> Vec<Row> {
    if ctx == Context::Host {
        return host::settings()
            .into_iter()
            .map(|s| {
                let note = Some(if s.live { "now" } else { "next match" });
                Row::Setting { s, note }
            })
            .collect();
    }
    let Some((name, settings)) = tabs.get(tab) else { return Vec::new() };
    let mut out: Vec<Row> = Vec::new();
    let binds = name == BINDS_TAB;
    if binds {
        out.push(Row::Section("Mouse".into()));
    }
    out.extend(settings.iter().cloned().map(|s| Row::Setting { s, note: None }));
    if binds {
        out.push(Row::Section("Keys".into()));
        out.push(Row::Games);
        for r in crate::controls::tab_rows(bind_game) {
            out.push(Row::Bind { ctab: bind_game, label: r.label, command: r.command });
        }
        // A game's numbers (the shared one, sensitivity, is a Mouse row).
        if bind_game > 0 {
            for n in crate::controls::tab_settings(bind_game) {
                let s = Setting {
                    label: n.label.to_string(),
                    cvar: n.cvar.to_string(),
                    pick: Pick::Range { min: n.min, max: n.max, step: n.step },
                    key: None,
                    help: String::new(),
                    live: true,
                    when: None,
                    widget: None,
                };
                out.push(Row::Setting { s, note: None });
            }
        }
    }
    out
}

/// What a click (or a key) does.
#[derive(Component, Message, Clone, Copy, Debug, PartialEq)]
pub enum SettingsAction {
    Tab(usize),
    /// A row's place (the pointer focusing it).
    Row(usize),
    Step { row: usize, dir: i8 },
    /// A choice's option (segmented, a dropdown's list).
    Pick { row: usize, option: usize },
    /// A slider's track (the press's place read from it).
    Slide(usize),
    /// A slider set to a place 0-1.
    SetFrac { row: usize, frac: f32 },
    OpenList(usize),
    Bind(usize),
    Clear(usize),
    Defaults,
    Apply,
    Revert,
    Back,
    Keep,
    KeepRevert,
}

/// The rows' scroll box.
#[derive(Component)]
struct SettingsList;

/// The screen's root.
#[derive(Component)]
pub struct SettingsRoot;

/// Its systems (before the main menu's and the Escape menu's: Esc).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsSystems;

/// Over the menus (the main menu's root and the Escape menu's).
pub const SETTINGS_Z: i32 = 92;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        crate::ui_cues::ensure_message(app);
        app.init_resource::<Settings>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .add_message::<SettingsAction>()
            .configure_sets(
                Update,
                SettingsSystems.before(crate::main_menu::MainMenuSystems).before(crate::esc_menu::EscMenuSystems),
            )
            .add_systems(
                Update,
                (follow, capture, keys, clicks, hover_focus, act, refresh_values, draw, wheel).chain().in_set(SettingsSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<SettingsAction>.in_set(SettingsSystems));
        use chimera_client::client::ui_scene::UiSceneHooks;
        UiSceneHooks::add(app.world_mut(), "settings", scene_settings);
        app.console_command("chimera:settings_press", "settings_press <apply|revert|back|keep>: press that button of Settings (tests).", |w, a| {
            let action = match a.first().map(String::as_str) {
                Some("apply") => SettingsAction::Apply,
                Some("revert") => SettingsAction::Revert,
                Some("back") => SettingsAction::Back,
                Some("keep") => SettingsAction::Keep,
                other => return Err(format!("no button {other:?}")),
            };
            w.write_message(action);
            Ok(None)
        });
    }
}

/// `ui_scene settings [tab]`: Settings over the menu (out of a game) or
/// over the match.
fn scene_settings(w: &mut World, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        w.resource_mut::<Settings>().stage = None;
        return close_scenes(w);
    }
    // Opened by `follow` once the menus are up (a run's start resets the
    // menu's page after the command line).
    let mut s = w.resource_mut::<Settings>();
    s.stage = Some(args.first().cloned().unwrap_or_default());
    s.ctx = None;
    Ok(())
}

/// Opens the page a staged scene asked for (`ui_scene settings`).
fn open_staged(world: &mut World) {
    let Some(tab) = world.resource::<Settings>().stage.clone() else { return };
    if context(world).is_some() {
        if world.resource::<Settings>().ctx.is_some() {
            world.resource_mut::<Settings>().stage = None;
        }
        return;
    }
    world.resource_mut::<Settings>().want_tab = Some(tab);
    let in_menu = world.get_resource::<crate::main_menu::PartyMenu>().is_some_and(|m| m.shown());
    if in_menu {
        let mut m = world.resource_mut::<crate::main_menu::PartyMenu>();
        m.failure = None;
        m.page = crate::main_menu::MenuPage::Options;
    } else {
        let mut e = world.resource_mut::<crate::esc_menu::EscMenu>();
        e.set_open(true);
        e.page = crate::esc_menu::Page::Options(0);
    }
}

fn close_scenes(w: &mut World) -> Result<(), String> {
    if let Some(mut e) = w.get_resource_mut::<crate::esc_menu::EscMenu>()
        && e.open
    {
        e.set_open(false);
    }
    Ok(())
}

/// Where Settings shows now (module doc).
pub fn context(world: &World) -> Option<Context> {
    if !signal(world.get_resource::<Theme>()) {
        return None;
    }
    if let Some(m) = world.get_resource::<crate::main_menu::PartyMenu>()
        && m.screen == crate::main_menu::Screen::MainMenu
        && m.page == crate::main_menu::MenuPage::Options
        && m.failure.is_none()
    {
        return Some(Context::Menu);
    }
    let e = world.get_resource::<crate::esc_menu::EscMenu>()?;
    match (e.open, e.page) {
        (true, crate::esc_menu::Page::Options(_)) => Some(Context::Game),
        (true, crate::esc_menu::Page::Host) => Some(Context::Host),
        _ => None,
    }
}

fn tabs_of(world: &World) -> Vec<(String, Vec<Setting>)> {
    crate::esc_menu::shown_tabs(world)
}

fn now(world: &World) -> f64 {
    world.get_resource::<UiClock>().map_or(0.0, |c| c.now)
}

fn submit(world: &mut World, line: impl Into<String>) {
    world.resource_mut::<Console>().submit(line.into());
}

fn cue(world: &mut World, c: UiCue) {
    crate::ui_cues::cue(world, c);
}

/// Opening, closing (leaving applies: the config written), the keep's
/// time running out.
fn follow(world: &mut World) {
    open_staged(world);
    let ctx = context(world);
    let t = now(world);
    let was = world.resource::<Settings>().ctx;
    world.resource_mut::<Settings>().esc_taken = false;
    if ctx != was {
        let (dirty, keep) = {
            let s = world.resource::<Settings>();
            (!s.baseline.is_empty() || s.binds_dirty, s.keep.clone())
        };
        if was.is_some() {
            // A display mode not kept goes back on leaving.
            if let Some(k) = keep {
                submit(world, format!("{} {}", k.cvar, chimera_core::console::quote(&k.old)));
            }
            if dirty && was != Some(Context::Host) {
                submit(world, "host_writeconfig");
            }
        }
        let tabs = tabs_of(world);
        let mut s = world.resource_mut::<Settings>();
        let want = s.want_tab.take();
        let stage = s.stage.take();
        *s = Settings { ctx, page_since: t, stage, ..default() };
        if let (Some(c), Some(w)) = (ctx, want) {
            let names = pages(c, &tabs);
            s.tab = names.iter().position(|n| n.eq_ignore_ascii_case(&w)).or_else(|| w.parse().ok()).unwrap_or(0).min(names.len().saturating_sub(1));
        }
        if let Some(c) = ctx {
            let rows = rows(c, &tabs, s.tab, 0);
            s.focus = rows.iter().position(Row::focusable).unwrap_or(0);
        }
        return;
    }
    let due = world.resource::<Settings>().keep.as_ref().is_some_and(|k| t >= k.until);
    if due {
        world.write_message(SettingsAction::KeepRevert);
    }
}

/// A bind row waiting: the next key, mouse button or wheel notch binds;
/// Esc cancels.
#[allow(clippy::too_many_arguments)]
fn capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    hovered: Query<&Interaction, With<SettingsAction>>,
    tabs: Option<Res<crate::esc_menu::OptionTabs>>,
    mut s: ResMut<Settings>,
    mut console: ResMut<Console>,
    mut cues: MessageWriter<UiCue>,
) {
    let (Some(ctx), Some(row)) = (s.ctx, s.capture) else { return };
    if keys.just_pressed(KeyCode::Escape) {
        s.capture = None;
        s.esc_taken = true;
        cues.write(UiCue::new("ui_back"));
        return;
    }
    let on_page = hovered.iter().any(|i| *i != Interaction::None);
    let mouse = if on_page { ButtonInput::<MouseButton>::default() } else { mouse.clone() };
    let Some(key) = binds::first_pressed(&keys, &mouse, Some(&scroll)) else { return };
    let tabs = tabs.map(|t| t.tabs.clone()).unwrap_or_default();
    if let Some(Row::Bind { ctab, command, .. }) = rows(ctx, &tabs, s.tab, s.bind_game).get(row) {
        let line = crate::controls::rebind_line(&console, *ctab, command, key);
        console.submit(line);
        s.binds_dirty = true;
        cues.write(UiCue::new("ui_select"));
    }
    s.capture = None;
}

/// The keys (module doc).
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    tabs: Option<Res<crate::esc_menu::OptionTabs>>,
    theme: Option<Res<Theme>>,
    mut s: ResMut<Settings>,
    mut out: MessageWriter<SettingsAction>,
    mut cues: MessageWriter<UiCue>,
) {
    let Some(ctx) = s.ctx else { return };
    let p = |k: KeyCode| keys.just_pressed(k);
    let enter = p(KeyCode::Enter) || p(KeyCode::NumpadEnter);
    if s.keep.is_some() {
        if enter {
            out.write(SettingsAction::Keep);
        } else if p(KeyCode::Escape) {
            s.esc_taken = true;
            out.write(SettingsAction::KeepRevert);
        }
        return;
    }
    if s.capture.is_some() || s.esc_taken {
        return;
    }
    let tabs = tabs.map(|t| t.tabs.clone()).unwrap_or_default();
    let rows = rows(ctx, &tabs, s.tab, s.bind_game);
    if let Some((row, act)) = s.open_list {
        let n = match rows.get(row) {
            Some(Row::Setting { s: set, .. }) => match &set.pick {
                Pick::Choices(c) => c.len(),
                _ => 1,
            },
            _ => 1,
        };
        if p(KeyCode::ArrowUp) || p(KeyCode::ArrowDown) {
            let d: i64 = if p(KeyCode::ArrowUp) { -1 } else { 1 };
            s.open_list = Some((row, (act as i64 + d).rem_euclid(n as i64) as usize));
            cues.write(UiCue::new("ui_hover"));
        } else if enter {
            out.write(SettingsAction::Pick { row, option: act });
        } else if p(KeyCode::Escape) {
            s.open_list = None;
            s.esc_taken = true;
            cues.write(UiCue::new("ui_back"));
        }
        return;
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && p(KeyCode::KeyS) {
        out.write(SettingsAction::Apply);
        return;
    }
    if ctrl && p(KeyCode::KeyZ) {
        out.write(SettingsAction::Revert);
        return;
    }
    let up = p(KeyCode::ArrowUp);
    if up || p(KeyCode::ArrowDown) {
        let live: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].focusable()).collect();
        if let Some(at) = live.iter().position(|i| *i == s.focus).or(Some(0)) {
            let d: i64 = if up { -1 } else { 1 };
            let next = live.get((at as i64 + d).clamp(0, live.len() as i64 - 1) as usize).copied().unwrap_or(0);
            if next != s.focus {
                s.focus = next;
                cues.write(UiCue::new("ui_hover"));
                if let Some(t) = theme.as_deref() {
                    let visible = visible_h(t);
                    s.scroll = scroll_to(t, &rows, next, s.scroll, visible);
                }
            }
        }
    }
    let focus = s.focus;
    if p(KeyCode::ArrowLeft) || p(KeyCode::ArrowRight) {
        out.write(SettingsAction::Step { row: focus, dir: if p(KeyCode::ArrowLeft) { -1 } else { 1 } });
    }
    if enter || p(KeyCode::Space) {
        match rows.get(focus) {
            Some(Row::Bind { .. }) => {
                out.write(SettingsAction::Bind(focus));
            }
            Some(Row::Setting { s: set, .. }) => match widget(set) {
                Widget::Dropdown => {
                    out.write(SettingsAction::OpenList(focus));
                }
                Widget::Toggle | Widget::Segmented | Widget::Stepper => {
                    out.write(SettingsAction::Step { row: focus, dir: 1 });
                }
                Widget::Slider { .. } => {}
            },
            Some(Row::Games) => {
                out.write(SettingsAction::Step { row: focus, dir: 1 });
            }
            _ => {}
        }
    }
    if (p(KeyCode::Delete) || p(KeyCode::Backspace)) && matches!(rows.get(focus), Some(Row::Bind { .. })) {
        out.write(SettingsAction::Clear(focus));
    }
    let n = pages(ctx, &tabs).len();
    if n > 1 && (p(KeyCode::KeyQ) || p(KeyCode::KeyE)) {
        let d: i64 = if p(KeyCode::KeyQ) { -1 } else { 1 };
        out.write(SettingsAction::Tab((s.tab as i64 + d).rem_euclid(n as i64) as usize));
    }
}

/// The rows' visible height, cq.
fn visible_h(t: &Theme) -> f32 {
    56.25 - t.layout(L, "win_top") - t.layout(L, "win_bottom") - 4.6 - 4.4
}

/// The scroll that keeps row `i` in view.
fn scroll_to(t: &Theme, rows: &[Row], i: usize, scroll: f32, visible: f32) -> f32 {
    let top: f32 = rows[..i].iter().map(|r| r.height(t)).sum();
    let bottom = top + rows.get(i).map_or(0.0, |r| r.height(t));
    let total: f32 = rows.iter().map(|r| r.height(t)).sum();
    let mut s = scroll;
    if bottom > s + visible {
        s = bottom - visible + 0.6;
    }
    if top < s {
        // A section header above the first row comes into view too.
        s = if i <= 1 { 0.0 } else { top - 0.6 };
    }
    s.clamp(0.0, (total - visible + 0.6).max(0.0))
}

/// Clicks as actions; a slider's press as its place.
fn clicks(
    pressed: Query<(&Interaction, &SettingsAction, Option<&RelativeCursorPosition>), Changed<Interaction>>,
    dragged: Query<(&Interaction, &SettingsAction, &RelativeCursorPosition)>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut last: Local<Option<(usize, f32)>>,
    mut out: MessageWriter<SettingsAction>,
) {
    for (i, a, pos) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        match (*a, pos) {
            (SettingsAction::Slide(row), Some(pos)) => {
                if let Some(f) = controls::slider_frac(pos) {
                    out.write(SettingsAction::SetFrac { row, frac: f });
                    *last = Some((row, f));
                }
            }
            (a, _) => {
                out.write(a);
            }
        }
    }
    // A drag along a pressed track.
    if mouse.pressed(MouseButton::Left) {
        for (i, a, pos) in &dragged {
            if let (Interaction::Pressed, SettingsAction::Slide(row)) = (i, a)
                && let Some(f) = controls::slider_frac(pos)
                && last.is_none_or(|(r, l)| r != *row || (l - f).abs() > 0.004)
            {
                out.write(SettingsAction::SetFrac { row: *row, frac: f });
                *last = Some((*row, f));
            }
        }
    } else {
        *last = None;
    }
}

/// The pointer on a row focuses it.
fn hover_focus(rows: Query<(&Interaction, &SettingsAction), Changed<Interaction>>, mut s: ResMut<Settings>) {
    if s.capture.is_some() || s.open_list.is_some() {
        return;
    }
    for (i, a) in &rows {
        if let (Interaction::Hovered, SettingsAction::Row(r)) = (i, a)
            && s.focus != *r
        {
            s.focus = *r;
        }
    }
}

/// Sets `cvar` to `value` (module doc: the baseline, the keep).
fn set_value(world: &mut World, ctx: Context, cvar: &str, value: &str) {
    let old = crate::controls::cvar_value(world, cvar).unwrap_or_default();
    let t = now(world);
    let keep_secs = world.get_resource::<Theme>().map_or(15.0, |th| th.layout(L, "keep_secs")) as f64;
    {
        let mut s = world.resource_mut::<Settings>();
        if ctx != Context::Host {
            let base = s.baseline.entry(cvar.to_string()).or_insert(old.clone()).clone();
            if host::same_value(&base, value) {
                s.baseline.remove(cvar);
            }
            if cvar == "chimera_fullscreen" && !host::same_value(&old, value) {
                let old = s.keep.as_ref().map_or(old.clone(), |k| k.old.clone());
                s.keep = Some(Keep { cvar: cvar.to_string(), old, until: t + keep_secs });
            }
        }
    }
    let quoted = if value.is_empty() { "\"\"".to_string() } else { chimera_core::console::quote(value) };
    submit(world, format!("{cvar} {quoted}"));
    world.resource_mut::<Settings>().values.insert(cvar.to_string(), value.to_string());
}

/// What the actions do.
fn act(world: &mut World) {
    let actions: Vec<SettingsAction> = world.resource_mut::<Messages<SettingsAction>>().drain().collect();
    for a in actions {
        act_one(world, a);
    }
}

fn act_one(world: &mut World, a: SettingsAction) {
    use SettingsAction as A;
    let Some(ctx) = world.resource::<Settings>().ctx else { return };
    let tabs = tabs_of(world);
    let (tab, bind_game) = {
        let s = world.resource::<Settings>();
        (s.tab, s.bind_game)
    };
    let rows = rows(ctx, &tabs, tab, bind_game);
    let setting = |row: usize| match rows.get(row) {
        Some(Row::Setting { s, .. }) => Some(s.clone()),
        _ => None,
    };
    match a {
        A::Tab(t) => {
            let n = pages(ctx, &tabs).len();
            if t < n && t != tab {
                let since = now(world);
                let rows = self::rows(ctx, &tabs, t, 0);
                let mut s = world.resource_mut::<Settings>();
                s.tab = t;
                s.bind_game = 0;
                s.focus = rows.iter().position(Row::focusable).unwrap_or(0);
                s.capture = None;
                s.open_list = None;
                s.scroll = 0.0;
                s.page_since = since;
                cue(world, UiCue::new("ui_select"));
            }
        }
        A::Row(r) => world.resource_mut::<Settings>().focus = r,
        A::Step { row, dir } => {
            world.resource_mut::<Settings>().focus = row;
            match rows.get(row) {
                Some(Row::Games) => {
                    let n = crate::controls::tab_count() as i64;
                    let mut s = world.resource_mut::<Settings>();
                    s.bind_game = (s.bind_game as i64 + dir as i64).rem_euclid(n) as usize;
                    s.capture = None;
                    cue(world, UiCue::new("ui_select"));
                }
                Some(Row::Setting { s, .. }) => {
                    let now = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
                    let next = s.pick.next(&now, dir);
                    cue(world, crate::ui_cues::step_cue(&s.pick, &next));
                    set_value(world, ctx, &s.cvar, &next);
                }
                _ => {}
            }
        }
        A::Pick { row, option } => {
            if let Some(s) = setting(row)
                && let Pick::Choices(c) = &s.pick
                && let Some((v, _)) = c.get(option)
            {
                world.resource_mut::<Settings>().open_list = None;
                cue(world, UiCue::new("ui_select"));
                set_value(world, ctx, &s.cvar, v);
            } else if let Some(Row::Games) = rows.get(row) {
                let mut st = world.resource_mut::<Settings>();
                st.bind_game = option.min(crate::controls::tab_count() - 1);
                st.focus = row;
                cue(world, UiCue::new("ui_select"));
            }
        }
        A::Slide(_) => {}
        A::SetFrac { row, frac } => {
            if let Some(s) = setting(row)
                && let Pick::Range { min, max, step } = s.pick
            {
                world.resource_mut::<Settings>().focus = row;
                let v = controls::value_at(frac, min, max, step);
                let now = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
                let next = format!("{v}");
                if !host::same_value(&now, &next) {
                    cue(world, crate::ui_cues::step_cue(&s.pick, &next));
                    set_value(world, ctx, &s.cvar, &next);
                }
            }
        }
        A::OpenList(row) => {
            let open = world.resource::<Settings>().open_list;
            let mut st = world.resource_mut::<Settings>();
            st.focus = row;
            if open.is_some_and(|(r, _)| r == row) {
                st.open_list = None;
                cue(world, UiCue::new("ui_back"));
            } else if let Some(s) = setting(row) {
                let at = match &s.pick {
                    Pick::Choices(c) => {
                        let v = world.resource::<Settings>().values.get(&s.cvar).cloned().unwrap_or_default();
                        c.iter().position(|(cv, _)| host::same_value(cv, &v)).unwrap_or(0)
                    }
                    _ => 0,
                };
                world.resource_mut::<Settings>().open_list = Some((row, at));
                cue(world, UiCue::new("ui_select"));
            }
        }
        A::Bind(row) => {
            let mut s = world.resource_mut::<Settings>();
            s.focus = row;
            s.capture = Some(row);
            cue(world, UiCue::new("ui_select"));
        }
        A::Clear(row) => {
            if let Some(Row::Bind { ctab, command, .. }) = rows.get(row) {
                let line = crate::controls::clear_line(world.resource::<Console>(), *ctab, command);
                if let Some(line) = line {
                    submit(world, line);
                    world.resource_mut::<Settings>().binds_dirty = true;
                }
                cue(world, UiCue::new("ui_back"));
            }
        }
        A::Defaults => {
            submit(world, crate::controls::defaults_line(bind_game));
            world.resource_mut::<Settings>().binds_dirty = true;
            cue(world, UiCue::new("ui_select"));
        }
        A::Apply => {
            let (n, binds) = {
                let s = world.resource::<Settings>();
                (s.baseline.len(), s.binds_dirty)
            };
            if n == 0 && !binds {
                cue(world, UiCue::new("ui_denied"));
                return;
            }
            submit(world, "host_writeconfig");
            let mut s = world.resource_mut::<Settings>();
            s.baseline.clear();
            s.binds_dirty = false;
            s.keep = None;
            cue(world, UiCue::new("ui_select"));
        }
        A::Revert => {
            let base = std::mem::take(&mut world.resource_mut::<Settings>().baseline);
            if base.is_empty() {
                cue(world, UiCue::new("ui_denied"));
                return;
            }
            for (cvar, old) in &base {
                let q = if old.is_empty() { "\"\"".to_string() } else { chimera_core::console::quote(old) };
                submit(world, format!("{cvar} {q}"));
            }
            world.resource_mut::<Settings>().keep = None;
            cue(world, UiCue::new("ui_back"));
        }
        A::Keep => {
            world.resource_mut::<Settings>().keep = None;
            cue(world, UiCue::new("ui_select"));
        }
        A::KeepRevert => {
            let keep = world.resource_mut::<Settings>().keep.take();
            if let Some(k) = keep {
                submit(world, format!("{} {}", k.cvar, chimera_core::console::quote(&k.old)));
                world.resource_mut::<Settings>().baseline.remove(&k.cvar);
                cue(world, UiCue::new("ui_back"));
            }
        }
        A::Back => match ctx {
            Context::Menu => {
                world.write_message(crate::main_menu::MainMenuAction::Back);
            }
            Context::Game | Context::Host => {
                world.write_message(crate::esc_menu::Action::Back);
            }
        },
    }
}

/// The page's values as the cvars hold them.
fn refresh_values(world: &mut World) {
    let Some(ctx) = world.resource::<Settings>().ctx else { return };
    let tabs = tabs_of(world);
    let (tab, game) = {
        let s = world.resource::<Settings>();
        (s.tab, s.bind_game)
    };
    for r in rows(ctx, &tabs, tab, game) {
        if let Row::Setting { s, .. } = r {
            let v = crate::controls::cvar_value(world, &s.cvar).unwrap_or_default();
            let mut st = world.resource_mut::<Settings>();
            if st.values.get(&s.cvar) != Some(&v) {
                st.values.insert(s.cvar.clone(), v);
            }
        }
    }
}

/// The mouse wheel over the rows scrolls them (not while a bind listens).
fn wheel(
    scroll: Res<AccumulatedMouseScroll>,
    theme: Option<Res<Theme>>,
    units: Option<Res<UiUnits>>,
    tabs: Option<Res<crate::esc_menu::OptionTabs>>,
    mut s: ResMut<Settings>,
    mut list: Query<(&mut ScrollPosition, &RelativeCursorPosition), With<SettingsList>>,
) {
    let (Some(ctx), Some(t), Some(u)) = (s.ctx, theme, units) else { return };
    if s.capture.is_some() || scroll.delta.y == 0.0 {
        return;
    }
    let Ok((mut pos, over)) = list.single_mut() else { return };
    if !over.cursor_over {
        return;
    }
    let tabs = tabs.map(|t| t.tabs.clone()).unwrap_or_default();
    let total: f32 = rows(ctx, &tabs, s.tab, s.bind_game).iter().map(|r| r.height(&t)).sum();
    let max = (total - visible_h(&t) + 0.6).max(0.0);
    let step = if scroll.unit == bevy::input::mouse::MouseScrollUnit::Line { 2.9 * scroll.delta.y } else { scroll.delta.y / u.cq(1.0) };
    let next = (s.scroll - step).clamp(0.0, max);
    if next != s.scroll {
        s.bypass_change_detection().scroll = next;
        pos.0.y = u.cq(next);
    }
}

/// What the screen last drew: drawn again when it differs.
#[derive(Default, PartialEq)]
struct DrawKey {
    s: Settings,
    keep_left: u64,
    binds: String,
    units: (u32, u32),
    theme: String,
    /// The menu's UI camera (drawn again once it is up).
    camera: Option<Entity>,
}

fn draw(world: &mut World, mut drawn: Local<Option<DrawKey>>) {
    let s = world.resource::<Settings>().clone();
    let roots: Vec<Entity> = world.query_filtered::<Entity, With<SettingsRoot>>().iter(world).collect();
    let Some(ctx) = s.ctx else {
        if drawn.take().is_some() {
            for r in roots {
                world.entity_mut(r).despawn();
            }
        }
        return;
    };
    let (Some(theme), Some(units)) = (world.get_resource::<Theme>().cloned(), world.get_resource::<UiUnits>().copied()) else { return };
    let t = now(world);
    let camera = if ctx == Context::Menu {
        world.query_filtered::<Entity, With<crate::main_menu::MenuUiCamera>>().iter(world).next()
    } else {
        None
    };
    let key = DrawKey {
        s: Settings { scroll: 0.0, esc_taken: false, ..s.clone() },
        keep_left: s.keep.as_ref().map_or(0, |k| (k.until - t).max(0.0).ceil() as u64),
        binds: {
            let c = world.resource::<Console>();
            format!("{:?}{:?}", c.binds, c.layers)
        },
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        theme: theme.name.clone(),
        camera,
    };
    if drawn.as_ref() == Some(&key) {
        return;
    }
    *drawn = Some(key);
    for r in roots {
        world.entity_mut(r).despawn();
    }
    let kit = world.get_resource::<KitFonts>().cloned();
    let tabs = tabs_of(world);
    let help = help_of(world, ctx, &tabs, &s);
    let keys_of: Vec<Vec<String>> = {
        let c = world.resource::<Console>();
        rows(ctx, &tabs, s.tab, s.bind_game)
            .iter()
            .map(|r| match r {
                Row::Bind { ctab, command, .. } => crate::controls::keys_on(c, *ctab, command).iter().map(|k| binds::display(k)).collect(),
                _ => Vec::new(),
            })
            .collect()
    };
    let mut images = world.remove_resource::<Assets<Image>>();
    let mut icons = world.remove_resource::<IconImages>();
    let mut ambients = world.remove_resource::<Assets<chimera_ui::AmbientMaterial>>();
    {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut c = Commands::new(&mut queue, world);
        let ui = Ui::new(&theme, kit.as_ref(), units);
        let d = Draw { ctx, s: &s, tabs: &tabs, help: &help, keys: &keys_of, now: t };
        let root = spawn(&mut c, &ui, &d, images.as_mut(), icons.as_mut(), ambients.as_mut());
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
    if let Some(a) = ambients {
        world.insert_resource(a);
    }
}

/// The focused row's help: title, text, key/values.
#[derive(Clone, Debug, Default, PartialEq)]
struct Help {
    title: String,
    text: String,
    kv: Vec<(&'static str, String)>,
}

fn range_text(p: &Pick) -> String {
    match p {
        Pick::Range { min, max, step } => format!("{min} – {max}, steps of {step}"),
        Pick::Choices(c) => c.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join(" / "),
    }
}

fn help_of(world: &World, ctx: Context, tabs: &[(String, Vec<Setting>)], s: &Settings) -> Help {
    let rows = rows(ctx, tabs, s.tab, s.bind_game);
    let console = world.resource::<Console>();
    match rows.get(s.focus) {
        Some(Row::Setting { s: set, note }) => {
            let cvar = console.cvar(&set.cvar);
            let text = if !set.help.is_empty() { set.help.clone() } else { cvar.map(|c| c.help.clone()).unwrap_or_default() };
            let mut kv = vec![("Console", set.cvar.clone())];
            if let Some(c) = cvar {
                kv.push(("Default", set.pick.shown(&c.default)));
            }
            kv.push(("Range", range_text(&set.pick)));
            if let Some(n) = note {
                kv.push(("Applies", (*n).to_string()));
            }
            Help { title: set.label.clone(), text, kv }
        }
        Some(Row::Bind { ctab, label, command }) => {
            let scope = if *ctab == 0 {
                "Every game.".to_string()
            } else {
                format!("{} only: replaces the shared key while you play it.", crate::controls::tab_label(*ctab))
            };
            Help {
                title: (*label).to_string(),
                text: format!("{scope} Enter or click, then press a key, a mouse button or move the wheel; Esc cancels; Delete clears."),
                kv: vec![("Console", (*command).to_string()), ("Range", "key, button or wheel".into())],
            }
        }
        Some(Row::Games) => Help {
            title: "Binds for".into(),
            text: "The shared keys work in every game; a game's own keys replace them while you play that game.".into(),
            kv: vec![("Console", "bind / bindgame".into())],
        },
        _ => Help::default(),
    }
}

struct Draw<'a> {
    ctx: Context,
    s: &'a Settings,
    tabs: &'a [(String, Vec<Setting>)],
    help: &'a Help,
    keys: &'a [Vec<String>],
    now: f64,
}

fn spawn(
    c: &mut Commands,
    ui: &Ui,
    d: &Draw,
    images: Option<&mut Assets<Image>>,
    icons: Option<&mut IconImages>,
    ambients: Option<&mut Assets<chimera_ui::AmbientMaterial>>,
) -> Entity {
    let t = ui.theme;
    let lay = |k: &str| t.layout(L, k);
    let s = d.s;
    let root = c
        .spawn((
            SettingsRoot,
            Name::new("settings"),
            GlobalZIndex(SETTINGS_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.tone_a(Tone::Ink, if d.ctx == Context::Menu { 1.0 } else { 0.86 })),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    if d.ctx == Context::Menu
        && let Some(amb) = ambients
    {
        let (col, bloom) = chimera_ui::AmbientMaterial::MENU;
        c.spawn((
            MaterialNode(amb.add(chimera_ui::AmbientMaterial::new(t).column(Some(col)).bloom(bloom))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    let frame = ui.boxed(c, root, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
    let m = lay("margin");
    // Header: the lockup, the title, where it is.
    let head = ui.boxed(c, frame, UiStyle { gap: Some(2.4), ..UiStyle::row() }.abs([Some(m), Some(2.8), Some(m), None]));
    if let (Some(images), Some(icons)) = (images, icons) {
        ui.lockup(c, head, images, icons, 1.05);
    }
    let (title, crumb) = match d.ctx {
        Context::Menu => ("Settings", "Main menu ▸ Settings"),
        Context::Game => ("Settings", "Esc menu ▸ Settings • the match goes on"),
        Context::Host => ("Host settings", "Esc menu ▸ Host settings • your server's"),
    };
    ui.text(c, head, Role::DisplayBold, 1.2, Tone::Fg, title);
    ui.text(c, head, Role::Mono, 0.5, Tone::Fg3, crumb);
    let rule = ui.boxed(c, frame, UiStyle { h: None, ..UiStyle::row() }.abs([Some(m), Some(7.4), Some(m), None]));
    c.spawn((Node { width: percent(100.0), height: Val::Px(ui.units.hairline()), ..default() }, BackgroundColor(ui.color(Tone::Fg4, 1.0)), ChildOf(rule)));

    // The window.
    let win = c
        .spawn((
            UiStyle::column().abs([Some(m), Some(lay("win_top")), Some(m), Some(lay("win_bottom"))]).bg_a(Tone::Ink, 0.78).outline(Tone::Fg5, 1.0),
            Node::default(),
            ChildOf(frame),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(ui.units.hairline()), ..default() },
        BackgroundColor(ui.color(Tone::Fg4, 1.0)),
        ChildOf(win),
    ));
    let names = pages(d.ctx, d.tabs);
    let tabbar = ui.boxed(c, win, UiStyle::column().abs([Some(2.0), Some(1.6), Some(2.0), None]));
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    for (i, e) in ui.tabs(c, tabbar, &name_refs, s.tab, false).into_iter().enumerate() {
        c.entity(e).insert(SettingsAction::Tab(i));
    }
    let cols = ui.boxed(c, win, UiStyle { gap: Some(2.4), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(2.0), Some(4.6), Some(2.0), Some(4.4)]));
    let list_box = ui.boxed(c, cols, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
    let list = c
        .spawn((
            UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), pad: Some([0.0, 1.0, 0.0, 0.0].into()), ..UiStyle::column() },
            Node::default(),
            SettingsList,
            RelativeCursorPosition::default(),
            ScrollPosition(Vec2::new(0.0, ui.units.cq(s.scroll))),
            ChildOf(list_box),
        ))
        .id();
    c.entity(list).insert(ScrollOverflow);
    let rows = rows(d.ctx, d.tabs, s.tab, s.bind_game);
    for (i, r) in rows.iter().enumerate() {
        draw_row(c, ui, list, i, r, d);
    }
    ui.wipe_cover(c, list_box, Tone::Ink, s.page_since, d.now, lay("wipe"));

    // The help pane.
    let help = ui.boxed(c, cols, UiStyle { w: Some(lay("help_w")), gap: Some(1.0), shrink: Some(0.0), ..UiStyle::column() });
    if !d.help.title.is_empty() {
        let tagrow = ui.boxed(c, help, UiStyle::row());
        let (node, label) = chimera_ui::widgets::tag(t, ui.kit, &ui.units, chimera_ui::widgets::TagKind::Accent, "Selected", 0.5);
        let tag = c.spawn((node, ChildOf(tagrow))).id();
        c.spawn((label, ChildOf(tag)));
        ui.text(c, help, Role::DisplayBold, 1.3, Tone::Fg, &d.help.title);
        let tx = ui.boxed(c, help, UiStyle { w_pct: Some(100.0), ..UiStyle::column() });
        ui.paragraph(c, tx, Role::Mono, 0.62, Tone::Fg2, &d.help.text);
        let kv: Vec<(&str, String)> = d.help.kv.iter().map(|(k, v)| (*k, v.clone())).collect();
        ui.kv(c, help, &kv, 6.0);
    }

    // The bar.
    let bar = c
        .spawn((
            UiStyle { h: Some(lay("bar_h")), gap: Some(1.2), pad: Some([0.0, 1.2, 0.0, 2.0].into()), ..UiStyle::row() }
                .abs([Some(0.0), None, Some(0.0), Some(0.0)])
                .bg_a(Tone::Ink, 0.6),
            Node::default(),
            ChildOf(win),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(ui.units.hairline()), ..default() },
        BackgroundColor(ui.color(Tone::Fg4, 1.0)),
        ChildOf(bar),
    ));
    let st = ui.boxed(c, bar, UiStyle { gap: Some(0.6), grow: Some(1.0), ..UiStyle::row() });
    let changed = s.baseline.len() + s.binds_dirty as usize;
    if d.ctx == Context::Host {
        ui.mark(c, st, Tone::Fg4, 0.42, 0.42);
        ui.text(c, st, Role::Mono, 0.6, Tone::Fg3, "Your server's settings: some now, some from the next match");
    } else {
        ui.mark(c, st, if changed > 0 { Tone::Acc } else { Tone::Fg4 }, 0.42, 0.42);
        let line = match changed {
            0 => "No changes".to_string(),
            1 => "1 change not applied".to_string(),
            n => format!("{n} changes not applied"),
        };
        ui.text(c, st, if changed > 0 { Role::Mono } else { Role::Mono }, 0.6, if changed > 0 { Tone::Fg } else { Tone::Fg3 }, &line);
    }
    let on_binds = names.get(s.tab).is_some_and(|n| n == "Controls");
    if on_binds {
        let b = ui.button(c, bar, Kind::Ghost, "Default keys", None, 2.0, State::NONE);
        c.entity(b).insert(SettingsAction::Defaults);
    }
    let back = ui.button(c, bar, Kind::Ghost, "Back", Some("Esc"), 2.0, State::NONE);
    c.entity(back).insert(SettingsAction::Back);
    if d.ctx != Context::Host {
        let off = State { disabled: changed == 0, ..State::NONE };
        let rv = ui.button(c, bar, Kind::Secondary, "Revert", Some("Ctrl Z"), 2.0, State { disabled: s.baseline.is_empty(), ..State::NONE });
        c.entity(rv).insert(SettingsAction::Revert);
        let ap = ui.button(c, bar, Kind::Primary, "Apply", Some("Ctrl S"), 2.0, off);
        c.entity(ap).insert(SettingsAction::Apply);
    }

    // The keys at the foot.
    let foot = ui.boxed(c, frame, UiStyle { gap: Some(2.0), ..UiStyle::row() }.abs([Some(m), None, None, Some(3.0)]));
    let mut keys = vec![("↑↓", "Row"), ("←→", "Change"), ("Enter", "Pick")];
    if names.len() > 1 {
        keys.push(("Q E", "Page"));
    }
    keys.push(("Esc", "Back"));
    for (k, what) in keys {
        let g = ui.boxed(c, foot, UiStyle { gap: Some(0.5), ..UiStyle::row() });
        ui.kbd(c, g, k, 0.6, Tone::Fg);
        ui.text(c, g, Role::Mono, 0.6, Tone::Fg2, what);
    }
    if let Some(k) = &s.keep {
        keep_modal(c, ui, root, (k.until - d.now).max(0.0).ceil() as u64);
    }
    root
}

/// The list scrolls (vertically) and clips.
#[derive(Component)]
struct ScrollOverflow;

fn draw_row(c: &mut Commands, ui: &Ui, list: Entity, i: usize, r: &Row, d: &Draw) {
    let t = ui.theme;
    let s = d.s;
    let focus = s.focus == i && s.open_list.is_none_or(|(o, _)| o == i);
    match r {
        Row::Section(title) => {
            ui.section(c, list, title, None);
        }
        Row::Games => {
            let (row, vl) = ui.row(c, list, "Binds for", None, t.layout(L, "row_h"), State::focused(focus));
            c.entity(row).insert(SettingsAction::Row(i));
            let names: Vec<&str> = (0..crate::controls::tab_count()).map(crate::controls::tab_label).collect();
            for (k, e) in ui.segmented(c, vl, &names, Some(s.bind_game), State::focused(focus)).into_iter().enumerate() {
                c.entity(e).insert(SettingsAction::Pick { row: i, option: k });
            }
        }
        Row::Bind { label, .. } => {
            let listening = s.capture == Some(i);
            let (row, vl) = ui.row(c, list, label, None, t.layout(L, "bind_h"), State::focused(focus));
            c.entity(row).insert(SettingsAction::Row(i));
            let keys = d.keys.get(i).cloned().unwrap_or_default();
            let f = ui.key_field(c, vl, &keys, listening, 8.5, State::focused(focus));
            c.entity(f).insert(SettingsAction::Bind(i));
            if !listening {
                let clr = ui.button(c, vl, Kind::Ghost, "Clear", None, 1.6, State::NONE);
                c.entity(clr).insert(SettingsAction::Clear(i));
            }
        }
        Row::Setting { s: set, note } => {
            let v = s.values.get(&set.cvar).cloned().unwrap_or_default();
            let changed = s.baseline.get(&set.cvar).is_some_and(|b| !host::same_value(b, &v));
            let st = State { focus, changed, ..State::NONE };
            let (row, vl) = ui.row(c, list, &set.label, *note, t.layout(L, "row_h"), st);
            c.entity(row).insert(SettingsAction::Row(i));
            let vst = State::focused(focus);
            match (widget(set), &set.pick) {
                (Widget::Slider { min, max, .. }, _) => {
                    let f = v.trim().parse::<f32>().map_or(0.0, |x| controls::frac_of(x, min, max));
                    let tr = ui.slider(c, vl, f, None, &set.pick.shown(&v), 15.0, vst);
                    c.entity(tr).insert(SettingsAction::Slide(i));
                }
                (Widget::Toggle, Pick::Choices(ch)) => {
                    let on = ch.get(1).is_some_and(|(cv, _)| host::same_value(cv, &v));
                    let tg = ui.toggle(c, vl, on, vst);
                    c.entity(tg).insert(SettingsAction::Step { row: i, dir: 1 });
                }
                (Widget::Segmented, Pick::Choices(ch)) => {
                    let labels: Vec<&str> = ch.iter().map(|(_, l)| l.as_str()).collect();
                    let sel = ch.iter().position(|(cv, _)| host::same_value(cv, &v));
                    for (k, e) in ui.segmented(c, vl, &labels, sel, vst).into_iter().enumerate() {
                        c.entity(e).insert(SettingsAction::Pick { row: i, option: k });
                    }
                }
                (Widget::Dropdown, Pick::Choices(ch)) => {
                    let labels: Vec<&str> = ch.iter().map(|(_, l)| l.as_str()).collect();
                    let sel = ch.iter().position(|(cv, _)| host::same_value(cv, &v)).unwrap_or(0);
                    let open = s.open_list.filter(|(r, _)| *r == i).map(|(_, a)| a);
                    let (f, opts) = ui.dropdown(c, vl, &labels, sel, open, 12.0, vst);
                    c.entity(f).insert(SettingsAction::OpenList(i));
                    for (k, e) in opts.into_iter().enumerate() {
                        c.entity(e).insert(SettingsAction::Pick { row: i, option: k });
                    }
                }
                _ => {
                    let (minus, plus) = ui.stepper(c, vl, &set.pick.shown(&v), 7.0, vst);
                    c.entity(minus).insert(SettingsAction::Step { row: i, dir: -1 });
                    c.entity(plus).insert(SettingsAction::Step { row: i, dir: 1 });
                }
            }
        }
    }
}

/// "Keep this display mode?": Keep, else back by itself.
fn keep_modal(c: &mut Commands, ui: &Ui, root: Entity, left: u64) {
    let shade = c
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui.color(Tone::Ink, 0.72)),
            bevy::ui::FocusPolicy::Block,
            ZIndex(30),
            ChildOf(root),
        ))
        .id();
    let m = ui.modal(c, shade, 30.0, false);
    ui.text(c, m, Role::DisplayBold, 1.35, Tone::Fg, "Keep this display mode?");
    let big = ui.boxed(c, m, UiStyle { gap: Some(0.8), margin: Some([1.0, 0.0, 0.3, 0.0].into()), ..UiStyle::row() }.align(AlignItems::Baseline));
    ui.text(c, big, Role::DisplayBold, 2.6, Tone::Fg, &format!("{left}"));
    ui.text(c, big, Role::Mono, 0.62, Tone::Fg2, "s, then it goes back");
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let k = ui.button(c, acts, Kind::Primary, "Keep", Some("Enter"), 2.4, State::focused(true));
    c.entity(k).insert(SettingsAction::Keep);
    let r = ui.button(c, acts, Kind::Secondary, "Revert", Some("Esc"), 2.4, State::NONE);
    c.entity(r).insert(SettingsAction::KeepRevert);
}

/// The list's overflow: scroll on y.
fn scroll_overflow(mut q: Query<&mut Node, (With<ScrollOverflow>, Changed<Node>)>) {
    for mut n in &mut q {
        if n.overflow != Overflow::scroll_y() {
            n.overflow = Overflow::scroll_y();
        }
    }
}

/// The overflow after the kit's styles (which write the `Node`).
pub struct SettingsOverflowPlugin;

impl Plugin for SettingsOverflowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, scroll_overflow.after(chimera_ui::UiKitSystems).before(bevy::ui::UiSystems::Layout));
    }
}
