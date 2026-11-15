//! Settings in the Signal look (feature `present`; a theme with
//! `[layout.settings]`, the Ultimate Gun Game's; docs/plans/active/
//! ugg-ui.md M7 and "Signal revision 8", the components page's frame
//! 02): one page for the main menu's Settings, the Escape menu's and the
//! Controls page (F6), and the Escape menu's Host settings in the same
//! window, on the shell (`chimera_ui::shell`: the bar with the breadcrumb
//! back to where it opened from, its Settings tile white; Back is the
//! breadcrumb, Esc or a right click, never while a bind listens: the
//! shell's `BackGuard`).
//!
//! - **When**: the main menu's Options page (`PartyMenu::page`) over the
//!   menu's ambient (`AmbientMaterial::MENU`), or the Escape menu's
//!   Options or Host settings page over the match (a flat 86 % shade), or
//!   the pre-match lobby's Settings button over the lobby (`EscMenu::
//!   lobby`: the same shade, "Lobby ▸ Settings", Back and Esc closing it;
//!   the match starting under it keeps it open, nothing lost):
//!   `Context`, from those menus' state, so their Esc, Back and actions
//!   keep working. Without the Signal theme those menus draw their own
//!   pages as before.
//! - **Pages** (sub-tabs, Q/E): data/options.toml's tabs, its Mouse tab
//!   becoming **Controls** with the Controls page's key binds under it (a
//!   segmented switch Shared / each game, `controls::GAMES`, each game's
//!   gun view FOV too); Host settings has one page, `host::settings()`,
//!   each row noting "now" or "next match", for the host or an RCON
//!   admin (the server's values, set through `rcon`: `host::value`,
//!   `host::set`). Only settings that exist.
//! - **Rows** by the setting's pick: a range is a slider (a press on the
//!   track jumps there and drags, the page drawn again under it with the
//!   drag kept in `Settings::slide`; the value shows and a preview's cvar
//!   follows while dragging, and it becomes pending once, at the release,
//!   with one slider cue; ←→ a step, a cue each; a click on its value box
//!   types a number, `Settings::edit`: Enter or a click away keeps it,
//!   Esc puts it back; any finite number is kept, past the range too (the
//!   thumb stops at the end), unless outside the setting's `hard_min` /
//!   `hard_max` (options.toml), which shows why beside the box), two choices Off/On a toggle, `widget =
//!   "dropdown"` a dropdown, up to five choices segmented, more a stepper;
//!   a bind a key field (Enter or click, then a key, a mouse button or
//!   the wheel; Esc cancels; Delete clears). ↑↓ move the focus (the
//!   pointer moves it too, until a key takes the lead and again once it
//!   moves: `chimera_ui::PointerLead`), the focused row's help in the pane at the
//!   right: what it does (the TOML `help`, else the cvar's own), its
//!   console name, its default and its range.
//! - **Apply / Revert** (the bar, Ctrl+S / Ctrl+Z), a deferred model: a
//!   change is pending (`Settings::pending`, its row marked changed in
//!   cobalt, the bar counting them) and reaches the cvar only on Apply,
//!   which sets every pending cvar and writes the config
//!   (`host_writeconfig`); Revert drops them. Volume and sensitivity
//!   sliders (`previews`) set their cvar while changed so they can be
//!   heard and felt, and are set back on Revert or Discard. Key binds join
//!   it: a rebind, Clear or Default keys goes to the console's bind table
//!   at once (the next rebind reads it; nothing plays while the page is
//!   up) over a snapshot taken before the first one (`bind_base`), which
//!   Revert and Discard put back; each key that differs counts as a
//!   change. Leaving (Esc, Back) with pending changes asks Apply / Discard
//!   / Stay in a modal (←→, Enter, Esc stays); anything else closing the
//!   page (a match ending, a disconnect) discards. A display-mode change
//!   (`chimera_fullscreen`) applied asks to be kept and goes back by
//!   itself after `layout.settings.keep_secs` (15 s), the config written
//!   again. Host settings stay immediate (the host's own cvars or rcon).
//! - **Scrolling**: the rows sit in the kit's scroll container
//!   (`chimera_ui::scroll`, key `SCROLL_KEY`): its bar, the eased wheel,
//!   the right-button pan with its fling; ↑↓ keep the focused row in view
//!   (`Settings::reveal`, the pointer moving the focus doesn't scroll); a
//!   page change starts at the top; locked while a bind listens or a
//!   list or a modal is up.
//! - **Motion**: a page change wipes the rows in on the 12° slant
//!   (`controls::Ui::wipe_cover`); screens change through the logo pass
//!   as before.
//! - **Sounds** (`ui_cues`): hover, select, back, a refused press and the
//!   slider's step.
//!
//! `ui_scene settings [tab]` and `ui_scene esc` stage them for
//! screenshots (`chimera_client::client::ui_scene`'s app hooks).
//!
//! Known shortcuts: pending binds sit in the console's table while the
//! page is up (restored from the snapshot, not held apart); the dropdown's list has no scroll (the
//! options' lists are short); the help pane has no previews (FOV fan,
//! crosshair); a key already bound elsewhere isn't flagged.

use std::collections::BTreeMap;

use bevy::{
    input::{
        ButtonInput, ButtonState,
        keyboard::KeyboardInput,
        mouse::AccumulatedMouseScroll,
    },
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, RelativeCursorPosition},
};
use chimera_client::client::{binds, ui_sound::UiCue};
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_ui::{
    IconImages, KitFonts, Role, ScrollViews, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{self, Kind, NOTE, SENTENCE, State, Ui},
    scroll::ScrollLocked,
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
    /// Applied from the leave prompt: Keep then leaves.
    pub then_leave: bool,
}

/// The console's bind table and its game layers (`bind_base`).
pub type BindTables = (BTreeMap<String, String>, BTreeMap<String, BTreeMap<String, String>>);

/// The leave prompt's buttons, left to right.
pub const LEAVE_CHOICES: [SettingsAction; 3] = [SettingsAction::LeaveApply, SettingsAction::LeaveDiscard, SettingsAction::LeaveStay];

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
    /// Each pending setting's new value, set on Apply (module doc).
    pub pending: BTreeMap<String, String>,
    /// Each pending setting's value before (the applied one).
    pub baseline: BTreeMap<String, String>,
    /// The binds before the first pending bind change (module doc).
    pub bind_base: Option<BindTables>,
    pub keep: Option<Keep>,
    /// The leave prompt (Apply / Discard / Stay): its focused button.
    pub leave: Option<usize>,
    /// When the page last changed (the wipe), on the UI clock.
    pub page_since: f64,
    /// Esc was this screen's this frame (a capture, a list, the keep):
    /// the menus leave it.
    pub esc_taken: bool,
    /// The shown settings' values as the cvars hold them.
    pub values: BTreeMap<String, String>,
    /// The keys moved the focus: its row is kept in view (the kit's
    /// scroll container, `chimera_ui::scroll`); the pointer moving it
    /// doesn't scroll.
    pub reveal: bool,
    /// A slider dragged (module doc).
    pub slide: Option<Slide>,
    /// A value box typed into (module doc).
    pub edit: Option<ValueEdit>,
    /// The toggle flipped last: its row and when (the UI clock), for its
    /// slab's slide.
    pub flip: Option<(usize, f64)>,
    /// A page asked for by name (`ui_scene settings <tab>`) at opening.
    pub want_tab: Option<String>,
    /// A staged scene's page, opened once the menus are up.
    pub stage: Option<String>,
}

/// A slider dragged: its row and its value at the press.
#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub row: usize,
    pub from: String,
}

/// A slider's value box typed into: the text, the caret (a char index),
/// why the text was refused.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueEdit {
    pub row: usize,
    pub text: String,
    pub caret: usize,
    /// The selection's other end (a char index; the caret's: none).
    pub anchor: usize,
    pub error: Option<String>,
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
                    hard_min: None,
                    hard_max: None,
                    percent: false,
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
    /// A slider set to a place 0-1 at once (pending, its cue).
    SetFrac { row: usize, frac: f32 },
    /// A press on a slider's track: the value jumps there and a drag
    /// starts; `SlideTo` follows the pointer (shown, previewed, not yet
    /// pending); `SlideEnd` (the button let go) makes it pending, its cue
    /// once.
    SlideStart { row: usize, frac: f32 },
    SlideTo { row: usize, frac: f32 },
    SlideEnd(usize),
    /// A slider's value box clicked: typing a number into it.
    EditValue(usize),
    /// The number typed kept (Enter, a click away), or refused.
    CommitEdit,
    OpenList(usize),
    Bind(usize),
    Clear(usize),
    Defaults,
    Apply,
    Revert,
    Back,
    Keep,
    KeepRevert,
    /// The leave prompt's buttons.
    LeaveApply,
    LeaveDiscard,
    LeaveStay,
}

/// A slider that sets its cvar while changed (volumes, sensitivity),
/// set back on Revert or Discard.
pub fn previews(s: &Setting) -> bool {
    matches!(s.pick, Pick::Range { .. }) && (s.cvar.contains("volume") || s.cvar.contains("sensitivity"))
}

/// The keys whose binding differs between `base` and the console's.
pub fn bind_changes(base: Option<&BindTables>, console: &Console) -> usize {
    let Some((shared, layers)) = base else { return 0 };
    fn diff(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> usize {
        let blank = |m: &BTreeMap<String, String>, k: &String| m.get(k).filter(|v| !v.is_empty()).cloned();
        a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))).filter(|k| blank(a, k) != blank(b, k)).count()
    }
    let empty = BTreeMap::new();
    let names: std::collections::BTreeSet<&String> = layers.keys().chain(console.layers.keys()).collect();
    diff(shared, &console.binds) + names.into_iter().map(|n| diff(layers.get(n).unwrap_or(&empty), console.layers.get(n).unwrap_or(&empty))).sum::<usize>()
}

impl Settings {
    /// Changes not applied: pending settings and differing keys.
    pub fn changes(&self, console: &Console) -> usize {
        self.pending.len() + bind_changes(self.bind_base.as_ref(), console)
    }
}

/// The rows' scroll view.
#[derive(Component)]
pub struct SettingsList;

/// The rows' view's key in `chimera_ui::ScrollViews`.
pub const SCROLL_KEY: &str = "settings";

/// The screen's root.
#[derive(Component)]
pub struct SettingsRoot;

/// Its systems (before the main menu's and the Escape menu's: Esc).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsSystems;

/// Over the menus (the main menu's root and the Escape menu's).
pub const SETTINGS_Z: i32 = 92;

/// The shade over the pre-match lobby (dimmed, still readable: its
/// countdown and rows go on under Settings).
pub const LOBBY_SHADE: f32 = 0.86;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        crate::ui_cues::ensure_message(app);
        chimera_ui::pointer_lead::add(app);
        app.init_resource::<Settings>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseScroll>()
            .add_message::<SettingsAction>()
            .add_message::<KeyboardInput>()
            .configure_sets(
                Update,
                SettingsSystems.before(crate::main_menu::MainMenuSystems).before(crate::esc_menu::EscMenuSystems),
            )
            .add_systems(
                Update,
                (follow, capture, edit_typing, keys, clicks, edit_pointer, hover_focus, act, refresh_values, draw).chain().in_set(SettingsSystems),
            )
            .add_systems(Update, crate::ui_cues::hover::<SettingsAction>.in_set(SettingsSystems))
            .add_systems(Update, hold_back);
        use chimera_client::client::ui_scene::UiSceneHooks;
        UiSceneHooks::add(app.world_mut(), "settings", scene_settings);
        app.console_command("chimera:settings_press", "settings_press <apply|revert|back|keep|revert_keep|leave_apply|leave_discard|stay|commit>: press that button of Settings (tests, screenshots).", |w, a| {
            let action = match a.first().map(String::as_str) {
                Some("apply") => SettingsAction::Apply,
                Some("revert") => SettingsAction::Revert,
                Some("back") => SettingsAction::Back,
                Some("keep") => SettingsAction::Keep,
                Some("revert_keep") => SettingsAction::KeepRevert,
                Some("leave_apply") => SettingsAction::LeaveApply,
                Some("leave_discard") => SettingsAction::LeaveDiscard,
                Some("stay") => SettingsAction::LeaveStay,
                Some("commit") => SettingsAction::CommitEdit,
                other => return Err(format!("no button {other:?}")),
            };
            w.write_message(action);
            Ok(None)
        });
        app.console_command("chimera:settings_type", "settings_type [text]: type into the value box (the focused row's, opened if none is; no text: just open it, its value selected) (tests, screenshots).", |w, a| {
            let text = a.join(" ");
            if w.resource::<Settings>().edit.is_none() {
                // The focused row's box opens first.
                let focus = w.resource::<Settings>().focus;
                act_one(w, SettingsAction::EditValue(focus));
            }
            if a.is_empty() {
                return Ok(None);
            }
            let mut s = w.resource_mut::<Settings>();
            let e = s.edit.as_mut().ok_or("the focused row takes no typed number")?;
            e.caret = text.chars().count();
            e.anchor = e.caret;
            e.text = text;
            e.error = None;
            Ok(None)
        });
        app.console_command("chimera:settings_set", "settings_set <cvar> <value>: change a setting on the open Settings page, pending until Apply (tests, screenshots).", |w, a| {
            let (Some(cvar), Some(value)) = (a.first(), a.get(1)) else { return Err("settings_set <cvar> <value>".into()) };
            let ctx = w.resource::<Settings>().ctx.ok_or("Settings isn't open")?;
            let shown = tabs_of(w);
            let set = shown
                .iter()
                .flat_map(|(_, ss)| ss.iter())
                .chain(crate::host::settings().iter())
                .find(|s| &s.cvar == cvar)
                .cloned()
                .ok_or_else(|| format!("no setting `{cvar}`"))?;
            let before = shown_value(w, ctx, &set.cvar);
            set_value(w, ctx, &set, value);
            // A toggle's slab slides, as a click's does.
            if widget(&set) == Widget::Toggle && !host::same_value(&before, value) {
                let at = tabs_of(w);
                let s = w.resource::<Settings>();
                let row = rows(ctx, &at, s.tab, s.bind_game).iter().position(|r| matches!(r, Row::Setting { s, .. } if s.cvar == set.cvar));
                if let Some(row) = row {
                    let since = now(w);
                    w.resource_mut::<Settings>().flip = Some((row, since));
                }
            }
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

/// Opening, closing (anything pending discarded: Esc and Back asked
/// first), the keep's time running out.
fn follow(world: &mut World) {
    open_staged(world);
    let ctx = context(world);
    let t = now(world);
    let was = world.resource::<Settings>().ctx;
    world.resource_mut::<Settings>().esc_taken = false;
    if ctx != was {
        if was.is_some() {
            // A display mode not kept goes back on leaving.
            revert_keep(world);
            discard(world);
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
        if s.bind_base.is_none() {
            s.bind_base = Some((console.binds.clone(), console.layers.clone()));
        }
        console.submit(line);
        cues.write(UiCue::new("ui_select"));
    }
    s.capture = None;
}

/// The keys (module doc).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    tabs: Option<Res<crate::esc_menu::OptionTabs>>,
    console: Res<Console>,
    mut s: ResMut<Settings>,
    mut out: MessageWriter<SettingsAction>,
    mut cues: MessageWriter<UiCue>,
) {
    let Some(ctx) = s.ctx else { return };
    let p = |k: KeyCode| keys.just_pressed(k);
    let enter = p(KeyCode::Enter) || p(KeyCode::NumpadEnter);
    if let Some(at) = s.leave {
        if p(KeyCode::ArrowLeft) || p(KeyCode::ArrowRight) {
            let d: i64 = if p(KeyCode::ArrowLeft) { -1 } else { 1 };
            let next = (at as i64 + d).clamp(0, LEAVE_CHOICES.len() as i64 - 1) as usize;
            if next != at {
                s.leave = Some(next);
                cues.write(UiCue::new("ui_hover"));
            }
        } else if enter {
            out.write(LEAVE_CHOICES[at]);
        } else if p(KeyCode::Escape) {
            s.esc_taken = true;
            out.write(SettingsAction::LeaveStay);
        }
        return;
    }
    if s.keep.is_some() {
        if enter {
            out.write(SettingsAction::Keep);
        } else if p(KeyCode::Escape) {
            s.esc_taken = true;
            out.write(SettingsAction::KeepRevert);
        }
        return;
    }
    if s.edit.is_some() {
        // Typing a number: Enter keeps it, Esc puts the value back.
        if enter {
            out.write(SettingsAction::CommitEdit);
        } else if p(KeyCode::Escape) {
            s.edit = None;
            s.esc_taken = true;
            cues.write(UiCue::new("ui_back"));
        }
        return;
    }
    if s.capture.is_some() || s.esc_taken {
        return;
    }
    // Leaving with changes pending asks first (the menus leave Esc).
    if p(KeyCode::Escape) && s.open_list.is_none() && ctx != Context::Host && s.changes(&console) > 0 {
        s.esc_taken = true;
        s.leave = Some(0);
        cues.write(UiCue::new("ui_select"));
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
                s.reveal = true;
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

/// Clicks as actions; a slider's press as its place and its drag.
fn clicks(
    pressed: Query<(&Interaction, &SettingsAction, Option<&RelativeCursorPosition>), Changed<Interaction>>,
    tracks: Query<(&SettingsAction, &RelativeCursorPosition)>,
    mouse: Res<ButtonInput<MouseButton>>,
    s: Res<Settings>,
    mut last: Local<Option<(usize, f32)>>,
    mut out: MessageWriter<SettingsAction>,
) {
    // A click away from the box typed into keeps what was typed.
    if let Some(e) = &s.edit
        && mouse.just_pressed(MouseButton::Left)
        && !pressed.iter().any(|(i, a, _)| *i == Interaction::Pressed && *a == SettingsAction::EditValue(e.row))
    {
        out.write(SettingsAction::CommitEdit);
    }
    for (i, a, pos) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        match (*a, pos) {
            (SettingsAction::Slide(row), Some(pos)) => {
                if let Some(f) = controls::slider_frac(pos) {
                    out.write(SettingsAction::SlideStart { row, frac: f });
                    *last = Some((row, f));
                }
            }
            (a, _) => {
                out.write(a);
            }
        }
    }
    // The drag: the page is drawn again as the value moves, so the track
    // is found by its row each frame (its `RelativeCursorPosition` holds
    // with the pointer outside it too).
    if let Some(sl) = &s.slide {
        if mouse.pressed(MouseButton::Left) {
            for (a, pos) in &tracks {
                if *a == SettingsAction::Slide(sl.row)
                    && let Some(f) = controls::slider_frac(pos)
                    && last.is_none_or(|(r, l)| r != sl.row || (l - f).abs() > 0.004)
                {
                    out.write(SettingsAction::SlideTo { row: sl.row, frac: f });
                    *last = Some((sl.row, f));
                }
            }
        } else {
            out.write(SettingsAction::SlideEnd(sl.row));
            *last = None;
        }
    } else if !mouse.pressed(MouseButton::Left) {
        *last = None;
    }
}

/// Keys typed into a value box (`Settings::edit`; Enter and Esc are
/// `keys`'): typing, the selection, copy, cut and paste
/// (`widgets::text_key`, the app's `Clipboard` else this system's).
fn edit_typing(
    mut events: MessageReader<KeyboardInput>,
    mut s: ResMut<Settings>,
    held: Option<Res<ButtonInput<KeyCode>>>,
    mut own: Local<chimera_ui::clipboard::Clipboard>,
    shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    use chimera_client::client::widgets::{Caret, text_key};
    if s.edit.is_none() {
        events.clear();
        return;
    }
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let held = |k: [KeyCode; 2]| held.as_ref().is_some_and(|h| h.any_pressed(k));
    let (ctrl, shift) = (held([KeyCode::ControlLeft, KeyCode::ControlRight]), held([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let Some(key) = text_key(&ev.logical_key, ev.key_code, ctrl, shift) else { continue };
        let Some(e) = s.edit.as_mut() else { return };
        let mut caret = Caret { at: e.caret, anchor: e.anchor };
        if caret.key(&mut e.text, key, clipboard, 24, false) {
            e.error = None;
        }
        e.caret = caret.at;
        e.anchor = caret.anchor;
    }
}

/// The pointer in a value box typed into (`controls::TextPress`): a
/// press places the caret between the characters it is nearest
/// (`controls::TextHits`, from the layout), a drag from it selects, a
/// double press selects all. The press that opens the box selects all
/// of it (`EditValue`), a drag from it selects from where it was.
fn edit_pointer(
    mouse: Res<ButtonInput<MouseButton>>,
    boxes: Query<(Entity, &Interaction, &SettingsAction), With<controls::EditText>>,
    hits: controls::TextHits,
    clock: Option<Res<UiClock>>,
    mut press: Local<(controls::TextPress, bool)>,
    mut s: ResMut<Settings>,
) {
    let now = clock.map_or(0.0, |c| c.now);
    let (press, dragged) = &mut *press;
    if mouse.just_pressed(MouseButton::Left) {
        *dragged = false;
        let hit = boxes.iter().find_map(|(e, i, a)| match a {
            SettingsAction::EditValue(row) if *i == Interaction::Pressed => Some((e, *row)),
            _ => None,
        });
        let Some((e, row)) = hit else {
            press.release();
            return;
        };
        let (Some(at), Some(len)) = (hits.char_at(e), hits.len(e)) else { return };
        let (c, a) = press.press(now, at, len);
        if let Some(ed) = s.edit.as_mut().filter(|ed| ed.row == row) {
            let n = ed.text.chars().count();
            ed.caret = c.min(n);
            ed.anchor = a.min(n);
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        press.release();
        return;
    }
    let Some(row) = s.edit.as_ref().map(|e| e.row) else { return };
    let Some(e) = boxes.iter().find_map(|(e, _, a)| (*a == SettingsAction::EditValue(row)).then_some(e)) else { return };
    let Some(at) = hits.char_at(e) else { return };
    if let Some((c, a)) = press.drag(at)
        && (c != a || *dragged)
    {
        *dragged = true;
        let ed = s.edit.as_mut().expect("checked");
        let n = ed.text.chars().count();
        let (c, a) = (c.min(n), a.min(n));
        if (ed.caret, ed.anchor) != (c, a) {
            ed.caret = c;
            ed.anchor = a;
        }
    }
}

/// The pointer on a row focuses it while the pointer leads
/// (`chimera_ui::PointerLead`: ↑↓ take the lead, so a still pointer, or a
/// row redrawn or scrolled under it, never pins the focus; a real move
/// gives it back and focuses the row under it).
fn hover_focus(
    rows: Query<(Ref<Interaction>, &SettingsAction)>,
    lead: Option<Res<chimera_ui::PointerLead>>,
    mut s: ResMut<Settings>,
) {
    if s.capture.is_some() || s.open_list.is_some() || s.leave.is_some() || s.keep.is_some() || s.slide.is_some() {
        return;
    }
    if lead.as_ref().is_some_and(|l| !l.hover()) {
        return;
    }
    let lead_back = lead.as_ref().is_some_and(|l| l.is_changed());
    for (i, a) in &rows {
        if let (Interaction::Hovered, SettingsAction::Row(r)) = (*i, a)
            && (i.is_changed() || lead_back)
            && s.focus != *r
        {
            s.focus = *r;
            s.reveal = false;
        }
    }
}

fn set_line(cvar: &str, value: &str) -> String {
    let quoted = if value.is_empty() { "\"\"".to_string() } else { chimera_core::console::quote(value) };
    format!("{cvar} {quoted}")
}

/// The value a setting shows: its pending one, else the cvar's; a host
/// setting's is the server's (`host::value`: an RCON admin's copy).
fn shown_value(world: &mut World, ctx: Context, cvar: &str) -> String {
    if ctx == Context::Host {
        return host::value(world, cvar).unwrap_or_default();
    }
    match world.resource::<Settings>().pending.get(cvar) {
        Some(v) => v.clone(),
        None => crate::controls::cvar_value(world, cvar).unwrap_or_default(),
    }
}

/// Changes `set` to `value` (module doc): pending until Apply (a preview
/// set at once too); Host settings at once.
fn set_value(world: &mut World, ctx: Context, set: &Setting, value: &str) {
    let cvar = set.cvar.as_str();
    if ctx == Context::Host {
        // The host's own console, or an RCON admin's `rcon`.
        host::set(world, cvar, value);
        world.resource_mut::<Settings>().values.insert(cvar.to_string(), value.to_string());
        return;
    }
    let applied = match world.resource::<Settings>().baseline.get(cvar) {
        Some(b) => b.clone(),
        None => crate::controls::cvar_value(world, cvar).unwrap_or_default(),
    };
    {
        let mut s = world.resource_mut::<Settings>();
        if host::same_value(&applied, value) {
            s.pending.remove(cvar);
            s.baseline.remove(cvar);
        } else {
            s.baseline.insert(cvar.to_string(), applied);
            s.pending.insert(cvar.to_string(), value.to_string());
        }
        s.values.insert(cvar.to_string(), value.to_string());
    }
    if previews(set) {
        submit(world, set_line(cvar, value));
    }
}

/// A slider dragged to `frac`: shown, and previewed for a preview's
/// cvar (its applied value kept first, for Revert), pending only at the
/// release (`SlideEnd`).
fn slide_live(world: &mut World, ctx: Context, set: &Setting, frac: f32) {
    let Pick::Range { min, max, step } = set.pick else { return };
    let v = format!("{}", controls::value_at(frac, min, max, step));
    let cvar = set.cvar.as_str();
    if world.resource::<Settings>().values.get(cvar).is_some_and(|x| host::same_value(x, &v)) {
        return;
    }
    let preview = ctx != Context::Host && previews(set);
    let applied = (preview && !world.resource::<Settings>().baseline.contains_key(cvar))
        .then(|| crate::controls::cvar_value(world, cvar).unwrap_or_default());
    let mut s = world.resource_mut::<Settings>();
    s.values.insert(cvar.to_string(), v.clone());
    if let Some(a) = applied {
        s.baseline.insert(cvar.to_string(), a);
    }
    if preview {
        submit(world, set_line(cvar, &v));
    }
}

/// Sets every pending cvar and writes the config; a display mode changed
/// asks to be kept. False: nothing was pending.
fn apply(world: &mut World) -> bool {
    let changes = {
        let s = world.resource::<Settings>();
        s.changes(world.resource::<Console>())
    };
    if changes == 0 {
        return false;
    }
    let t = now(world);
    let keep_secs = world.get_resource::<Theme>().map_or(15.0, |th| th.layout(L, "keep_secs")) as f64;
    let (pending, baseline) = {
        let mut s = world.resource_mut::<Settings>();
        s.bind_base = None;
        (std::mem::take(&mut s.pending), std::mem::take(&mut s.baseline))
    };
    for (cvar, v) in &pending {
        submit(world, set_line(cvar, v));
        if cvar == "chimera_fullscreen" {
            let old = baseline.get(cvar).cloned().unwrap_or_default();
            world.resource_mut::<Settings>().keep = Some(Keep { cvar: cvar.clone(), old, until: t + keep_secs, then_leave: false });
        }
    }
    submit(world, "host_writeconfig");
    true
}

/// Drops what is pending: previews set back, the binds' snapshot put
/// back. False: nothing was pending.
fn discard(world: &mut World) -> bool {
    let (pending, baseline, base) = {
        let mut s = world.resource_mut::<Settings>();
        let p = std::mem::take(&mut s.pending);
        let b = std::mem::take(&mut s.baseline);
        (p, b, s.bind_base.take())
    };
    let tabs = tabs_of(world);
    let preview = |cvar: &str| tabs.iter().flat_map(|(_, ss)| ss.iter()).any(|s| s.cvar == cvar && previews(s));
    let mut any = !pending.is_empty();
    for (cvar, old) in &baseline {
        if preview(cvar) {
            submit(world, set_line(cvar, old));
        }
    }
    if let Some((shared, layers)) = base {
        let mut c = world.resource_mut::<Console>();
        any |= c.binds != shared || c.layers != layers;
        c.binds = shared;
        c.layers = layers;
    }
    any
}

/// A display mode waiting to be kept goes back (the config written again:
/// Apply wrote the new one).
fn revert_keep(world: &mut World) -> bool {
    let Some(k) = world.resource_mut::<Settings>().keep.take() else { return false };
    submit(world, set_line(&k.cvar, &k.old));
    submit(world, "host_writeconfig");
    true
}

/// Back (Esc or the button) to the menu Settings came from.
fn leave(world: &mut World, ctx: Context) {
    match ctx {
        Context::Menu => {
            world.write_message(crate::main_menu::MainMenuAction::Back);
        }
        Context::Game | Context::Host => {
            world.write_message(crate::esc_menu::Action::Back);
        }
    }
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
                s.reveal = false;
                s.page_since = since;
                if let Some(mut v) = world.get_resource_mut::<ScrollViews>() {
                    v.reset(SCROLL_KEY);
                }
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
                    let now = shown_value(world, ctx, &s.cvar);
                    let next = s.pick.next(&now, dir);
                    cue(world, crate::ui_cues::step_cue(&s.pick, &next));
                    set_value(world, ctx, s, &next);
                    if widget(s) == Widget::Toggle && !host::same_value(&now, &next) {
                        let since = self::now(world);
                        world.resource_mut::<Settings>().flip = Some((row, since));
                    }
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
                set_value(world, ctx, &s, v);
            } else if let Some(Row::Games) = rows.get(row) {
                let mut st = world.resource_mut::<Settings>();
                st.bind_game = option.min(crate::controls::tab_count() - 1);
                st.focus = row;
                cue(world, UiCue::new("ui_select"));
            }
        }
        A::Slide(_) => {}
        A::SlideStart { row, frac } => {
            if let Some(set) = setting(row)
                && matches!(set.pick, Pick::Range { .. })
            {
                let from = shown_value(world, ctx, &set.cvar);
                let mut st = world.resource_mut::<Settings>();
                st.focus = row;
                st.edit = None;
                st.slide = Some(Slide { row, from });
                slide_live(world, ctx, &set, frac);
            }
        }
        A::SlideTo { row, frac } => {
            if world.resource::<Settings>().slide.as_ref().is_some_and(|sl| sl.row == row)
                && let Some(set) = setting(row)
            {
                slide_live(world, ctx, &set, frac);
            }
        }
        A::SlideEnd(row) => {
            let sl = world.resource_mut::<Settings>().slide.take();
            if let Some(sl) = sl.filter(|sl| sl.row == row)
                && let Some(set) = setting(row)
            {
                let v = world.resource::<Settings>().values.get(&set.cvar).cloned().unwrap_or_else(|| sl.from.clone());
                set_value(world, ctx, &set, &v);
                // One cue for the whole drag (or the click), none for no change.
                if !host::same_value(&sl.from, &v) {
                    cue(world, crate::ui_cues::step_cue(&set.pick, &v));
                }
            }
        }
        A::EditValue(row) => {
            if let Some(set) = setting(row)
                && matches!(set.pick, Pick::Range { .. })
                && world.resource::<Settings>().edit.as_ref().is_none_or(|e| e.row != row)
            {
                let text = shown_value(world, ctx, &set.cvar).trim().to_string();
                // A percentage is typed as one (160 for 1.6).
                let text = match text.parse::<f32>() {
                    Ok(f) if set.percent => format!("{}", (f * 100.0).round()),
                    _ => text,
                };
                let mut st = world.resource_mut::<Settings>();
                st.focus = row;
                st.open_list = None;
                let n = text.chars().count();
                // All of it selected: what is typed replaces it.
                st.edit = Some(ValueEdit { row, caret: n, anchor: 0, text, error: None });
                cue(world, UiCue::new("ui_select"));
            }
        }
        A::CommitEdit => {
            let Some(e) = world.resource::<Settings>().edit.clone() else { return };
            let Some(set) = setting(e.row) else {
                world.resource_mut::<Settings>().edit = None;
                return;
            };
            // A percentage's box takes percent ("160" or "160%" is 1.6).
            let k = if set.percent { 100.0 } else { 1.0 };
            let typed = e.text.trim().trim_end_matches('%');
            match controls::typed_number(typed, (set.hard_min.map(|m| m * k), set.hard_max.map(|m| m * k))).map(|v| v / k) {
                Ok(v) => {
                    world.resource_mut::<Settings>().edit = None;
                    let next = format!("{v}");
                    let now = shown_value(world, ctx, &set.cvar);
                    if !host::same_value(&now, &next) {
                        set_value(world, ctx, &set, &next);
                    }
                    cue(world, UiCue::new("ui_select"));
                }
                Err(why) => {
                    if let Some(e) = world.resource_mut::<Settings>().edit.as_mut() {
                        e.error = Some(why);
                    }
                    cue(world, UiCue::new("ui_denied"));
                }
            }
        }
        A::SetFrac { row, frac } => {
            if let Some(s) = setting(row)
                && let Pick::Range { min, max, step } = s.pick
            {
                world.resource_mut::<Settings>().focus = row;
                let v = controls::value_at(frac, min, max, step);
                let now = shown_value(world, ctx, &s.cvar);
                let next = format!("{v}");
                if !host::same_value(&now, &next) {
                    cue(world, crate::ui_cues::step_cue(&s.pick, &next));
                    set_value(world, ctx, &s, &next);
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
                    snapshot_binds(world);
                    submit(world, line);
                }
                cue(world, UiCue::new("ui_back"));
            }
        }
        A::Defaults => {
            snapshot_binds(world);
            submit(world, crate::controls::defaults_line(bind_game));
            cue(world, UiCue::new("ui_select"));
        }
        A::Apply => {
            if ctx == Context::Host || !apply(world) {
                cue(world, UiCue::new("ui_denied"));
                return;
            }
            cue(world, UiCue::new("ui_select"));
        }
        A::Revert => {
            if ctx == Context::Host || !discard(world) {
                cue(world, UiCue::new("ui_denied"));
                return;
            }
            cue(world, UiCue::new("ui_back"));
        }
        A::Keep => {
            let keep = world.resource_mut::<Settings>().keep.take();
            cue(world, UiCue::new("ui_select"));
            if keep.is_some_and(|k| k.then_leave) {
                leave(world, ctx);
            }
        }
        A::KeepRevert => {
            if revert_keep(world) {
                cue(world, UiCue::new("ui_back"));
            }
        }
        A::Back => {
            let pending = ctx != Context::Host && world.resource::<Settings>().changes(world.resource::<Console>()) > 0;
            let mut s = world.resource_mut::<Settings>();
            if s.keep.is_some() || s.leave.is_some() {
                return;
            }
            if pending {
                s.leave = Some(0);
                s.capture = None;
                s.open_list = None;
                cue(world, UiCue::new("ui_select"));
            } else {
                leave(world, ctx);
            }
        }
        A::LeaveApply => {
            if world.resource_mut::<Settings>().leave.take().is_none() {
                return;
            }
            apply(world);
            cue(world, UiCue::new("ui_select"));
            // A display mode to keep: its prompt first, then out.
            match world.resource_mut::<Settings>().keep.as_mut() {
                Some(k) => k.then_leave = true,
                None => leave(world, ctx),
            }
        }
        A::LeaveDiscard => {
            if world.resource_mut::<Settings>().leave.take().is_none() {
                return;
            }
            discard(world);
            cue(world, UiCue::new("ui_back"));
            leave(world, ctx);
        }
        A::LeaveStay => {
            if world.resource_mut::<Settings>().leave.take().is_some() {
                cue(world, UiCue::new("ui_back"));
            }
        }
    }
}

/// The binds as they are before the first pending change (module doc).
fn snapshot_binds(world: &mut World) {
    if world.resource::<Settings>().bind_base.is_none() {
        let c = world.resource::<Console>();
        let base = (c.binds.clone(), c.layers.clone());
        world.resource_mut::<Settings>().bind_base = Some(base);
    }
}

/// The page's values as the cvars hold them.
fn refresh_values(world: &mut World) {
    let Some(ctx) = world.resource::<Settings>().ctx else { return };
    let tabs = tabs_of(world);
    let (tab, game, sliding) = {
        let s = world.resource::<Settings>();
        (s.tab, s.bind_game, s.slide.as_ref().map(|sl| sl.row))
    };
    for (i, r) in rows(ctx, &tabs, tab, game).into_iter().enumerate() {
        // A slider being dragged shows the drag, not the cvar.
        if sliding == Some(i) {
            continue;
        }
        if let Row::Setting { s, .. } = r {
            let v = shown_value(world, ctx, &s.cvar);
            let mut st = world.resource_mut::<Settings>();
            if st.values.get(&s.cvar) != Some(&v) {
                st.values.insert(s.cvar.clone(), v);
            }
        }
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
    /// Over the themed pre-match lobby (`EscMenu::lobby`).
    lobby: bool,
    party: chimera_ui::shell::Party,
}

fn draw(world: &mut World, mut drawn: Local<Option<DrawKey>>) {
    let s = world.resource::<Settings>().clone();
    let roots: Vec<Entity> = world.query_filtered::<Entity, With<SettingsRoot>>().iter(world).collect();
    let Some(ctx) = s.ctx else {
        if drawn.take().is_some() {
            for r in roots {
                world.entity_mut(r).despawn();
            }
            // Opened again: from the top.
            if let Some(mut v) = world.get_resource_mut::<ScrollViews>() {
                v.forget(SCROLL_KEY);
            }
        }
        return;
    };
    let (Some(theme), Some(units)) = (world.get_resource::<Theme>().cloned(), world.get_resource::<UiUnits>().copied()) else { return };
    let t = now(world);
    let lobby = ctx != Context::Menu && world.get_resource::<crate::esc_menu::EscMenu>().is_some_and(|m| m.lobby);
    let camera = if ctx == Context::Menu {
        world.query_filtered::<Entity, With<crate::main_menu::MenuUiCamera>>().iter(world).next()
    } else {
        None
    };
    let key = DrawKey {
        s: Settings { esc_taken: false, ..s.clone() },
        keep_left: s.keep.as_ref().map_or(0, |k| (k.until - t).max(0.0).ceil() as u64),
        binds: {
            let c = world.resource::<Console>();
            format!("{:?}{:?}", c.binds, c.layers)
        },
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
        theme: theme.name.clone(),
        camera,
        lobby,
        party: world.get_resource::<chimera_ui::shell::Party>().cloned().unwrap_or_default(),
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
    let (keys_of, bind_changed, changes): (Vec<Vec<String>>, Vec<bool>, usize) = {
        let c = world.resource::<Console>();
        let rows = rows(ctx, &tabs, s.tab, s.bind_game);
        let keys = rows
            .iter()
            .map(|r| match r {
                Row::Bind { ctab, command, .. } => crate::controls::keys_on(c, *ctab, command).iter().map(|k| binds::display(k)).collect(),
                _ => Vec::new(),
            })
            .collect();
        let changed = rows
            .iter()
            .map(|r| match (r, &s.bind_base) {
                (Row::Bind { ctab, command, .. }, Some((b, l))) => crate::controls::keys_in(b, l, *ctab, command) != crate::controls::keys_on(c, *ctab, command),
                _ => false,
            })
            .collect();
        (keys, changed, s.changes(c))
    };
    let mut images = world.remove_resource::<Assets<Image>>();
    let mut icons = world.remove_resource::<IconImages>();
    let mut ambients = world.remove_resource::<Assets<chimera_ui::AmbientMaterial>>();
    {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut c = Commands::new(&mut queue, world);
        let ui = Ui::new(&theme, kit.as_ref(), units);
        let reduced = world.get_resource::<chimera_ui::ReducedMotion>().is_some_and(|r| r.0);
        let party = drawn.as_ref().map(|k| k.party.clone()).unwrap_or_default();
        let d = Draw { ctx, lobby, s: &s, tabs: &tabs, help: &help, keys: &keys_of, bind_changed: &bind_changed, changes, now: t, reduced, party: &party };
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

/// A setting's range as the help pane says it: a percentage's in percent
/// (a volume's "0 – 200%, steps of 5%", never its 0.05 step).
pub fn range_text(set: &Setting) -> String {
    let p = &set.pick;
    match p {
        Pick::Range { min, max, step } if set.percent => {
            let pc = |v: f32| (v * 100.0).round();
            format!("{}% – {}%, steps of {}%", pc(*min), pc(*max), pc(*step))
        }
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
                kv.push(("Default", set.shown(&c.default)));
            }
            kv.push(("Range", range_text(set)));
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
    /// Over the themed pre-match lobby (its own words and shade).
    lobby: bool,
    s: &'a Settings,
    tabs: &'a [(String, Vec<Setting>)],
    help: &'a Help,
    keys: &'a [Vec<String>],
    /// Each bind row's keys differ from before the pending changes.
    bind_changed: &'a [bool],
    /// Changes not applied (`Settings::changes`).
    changes: usize,
    now: f64,
    /// Reduced motion: no toggle slide.
    reduced: bool,
    party: &'a chimera_ui::shell::Party,
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
            chimera_ui::shell::ShellScreen,
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
            BackgroundColor(t.tone_a(Tone::Ink, match d.ctx {
                Context::Menu => 1.0,
                _ if d.lobby => LOBBY_SHADE,
                _ => 0.86,
            })),
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
    let sh = chimera_ui::shell::ShellLayout::of(t);
    // The shell's bar: the breadcrumb back to where it was opened from
    // over the title, the party, the system tiles with Settings open.
    let (parent, title) = match d.ctx {
        Context::Menu => ("Main menu", "Settings"),
        Context::Game if d.lobby => ("Lobby", "Settings"),
        Context::Game => ("Match", "Settings"),
        Context::Host => ("Match", "Host settings"),
    };
    if let (Some(images), Some(icons)) = (images, icons) {
        let bar = chimera_ui::shell::Bar {
            left: chimera_ui::shell::Left::Crumb { parent, title },
            party: &d.party.members,
            party_max: d.party.max,
            open: (d.ctx != Context::Host).then_some(chimera_ui::shell::ShellTile::Settings),
        };
        chimera_ui::shell::bar(ui, c, frame, images, icons, &bar);
    }

    // The window.
    let win = chimera_ui::shell::window(ui, c, frame);
    let names = pages(d.ctx, d.tabs);
    let tabbar = ui.boxed(c, win, UiStyle::column().abs([Some(sh.rows_in), Some(1.6), Some(sh.side_w + 2.0 * sh.rows_in), None]));
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    for (i, e) in ui.tabs_ex(c, tabbar, &name_refs, s.tab, false, (name_refs.len() > 1).then_some(("Q", "E"))).into_iter().enumerate() {
        c.entity(e).insert(SettingsAction::Tab(i));
    }
    let cols = ui.boxed(c, win, UiStyle { gap: Some(2.0 * sh.rows_in), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(sh.rows_in), Some(4.6), Some(sh.rows_in), Some(sh.act_h + 1.4)]));
    // The rows scroll in the kit's container (its bar, the wheel, a
    // right-button pan); the keys keep the focused row in view.
    let sc = ui.scroll(c, cols, SCROLL_KEY, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() }, UiStyle::column());
    c.entity(sc.view).insert(SettingsList);
    if s.capture.is_some() || s.open_list.is_some() || s.leave.is_some() || s.keep.is_some() {
        // A bind listening takes the right button; a list or a modal is over it.
        c.entity(sc.view).insert(ScrollLocked);
    }
    let rows = rows(d.ctx, d.tabs, s.tab, s.bind_game);
    for (i, r) in rows.iter().enumerate() {
        let row = draw_row(c, ui, sc.content, i, r, d);
        if s.reveal && i == s.focus
            && let Some(row) = row
        {
            chimera_ui::scroll_into_view(c, row);
        }
    }
    ui.wipe_cover(c, sc.frame, Tone::Ink, s.page_since, d.now, lay("wipe"));

    // The help pane.
    let help = ui.boxed(c, cols, UiStyle { w: Some(sh.side_w), gap: Some(1.0), shrink: Some(0.0), ..UiStyle::column() });
    if !d.help.title.is_empty() {
        let tagrow = ui.boxed(c, help, UiStyle::row());
        let (node, label) = chimera_ui::widgets::tag(t, ui.kit, &ui.units, chimera_ui::widgets::TagKind::Accent, "Selected", 0.5);
        let tag = c.spawn((node, ChildOf(tagrow))).id();
        c.spawn((label, ChildOf(tag)));
        ui.text(c, help, Role::DisplayBold, 1.3, Tone::Fg, &d.help.title);
        let tx = ui.boxed(c, help, UiStyle { w_pct: Some(100.0), ..UiStyle::column() });
        ui.paragraph(c, tx, Role::Text, SENTENCE, Tone::Fg2, &d.help.text);
        // Console names in their own (lower) case, other values in capitals.
        let kv: Vec<(&str, String)> =
            d.help.kv.iter().map(|(k, v)| (*k, if *k == "Console" { v.clone() } else { v.to_uppercase() })).collect();
        ui.kv(c, help, &kv, 6.0);
    }

    // The action bar.
    let bar = chimera_ui::shell::action_bar(ui, c, win);
    let st = ui.boxed(c, bar, UiStyle { gap: Some(0.6), grow: Some(1.0), ..UiStyle::row() });
    let changed = d.changes;
    if d.ctx == Context::Host {
        ui.mark(c, st, Tone::Fg4, 0.42, 0.42);
        ui.sentence(c, st, NOTE, Tone::Fg3, "Your server's settings: some now, some from the next match");
    } else {
        ui.mark(c, st, if changed > 0 { Tone::Acc } else { Tone::Fg4 }, 0.42, 0.42);
        let line = match changed {
            0 => "No changes".to_string(),
            1 => "1 change not applied".to_string(),
            n => format!("{n} changes not applied"),
        };
        ui.sentence(c, st, NOTE, if changed > 0 { Tone::Fg } else { Tone::Fg3 }, &line);
    }
    let on_binds = names.get(s.tab).is_some_and(|n| n == "Controls");
    if on_binds {
        let b = ui.button(c, bar, Kind::Ghost, "Default keys", None, 2.0, State::NONE);
        c.entity(b).insert(SettingsAction::Defaults);
    }
    if d.ctx != Context::Host {
        let off = State { disabled: changed == 0, ..State::NONE };
        let rv = ui.button(c, bar, Kind::Secondary, "Revert", Some("Ctrl Z"), 2.0, off);
        c.entity(rv).insert(SettingsAction::Revert);
        let ap = ui.button(c, bar, Kind::Primary, "Apply", Some("Ctrl S"), 2.0, off);
        c.entity(ap).insert(SettingsAction::Apply);
    }

    // The keys at the foot (Back is the breadcrumb; Q and E sit at the
    // tab strip's ends).
    chimera_ui::shell::keys(ui, c, frame, &[("↑↓", "Row"), ("←→", "Change"), ("Enter", "Pick")]);
    if let Some(k) = &s.keep {
        keep_modal(c, ui, root, (k.until - d.now).max(0.0).ceil() as u64);
    } else if let Some(at) = s.leave {
        leave_modal(c, ui, root, at, changed);
    }
    root
}

/// Draws row `i`; its row node (none for a section).
fn draw_row(c: &mut Commands, ui: &Ui, list: Entity, i: usize, r: &Row, d: &Draw) -> Option<Entity> {
    let t = ui.theme;
    let s = d.s;
    let focus = s.focus == i && s.open_list.is_none_or(|(o, _)| o == i);
    match r {
        Row::Section(title) => {
            ui.section(c, list, title, None);
            None
        }
        Row::Games => {
            let (row, vl) = ui.row(c, list, "Binds for", None, t.layout(L, "row_h"), State::focused(focus));
            c.entity(row).insert((SettingsAction::Row(i), crate::ui_cues::CueItem(format!("settings:{}:{i}", s.tab))));
            let names: Vec<&str> = (0..crate::controls::tab_count()).map(crate::controls::tab_label).collect();
            for (k, e) in ui.segmented(c, vl, &names, Some(s.bind_game), State::focused(focus)).into_iter().enumerate() {
                c.entity(e).insert(SettingsAction::Pick { row: i, option: k });
            }
            Some(row)
        }
        Row::Bind { label, .. } => {
            let listening = s.capture == Some(i);
            let changed = d.bind_changed.get(i).copied().unwrap_or(false);
            let (row, vl) = ui.row(c, list, label, None, t.layout(L, "bind_h"), State { focus, changed, ..State::NONE });
            c.entity(row).insert((SettingsAction::Row(i), crate::ui_cues::CueItem(format!("settings:{}:{i}", s.tab))));
            let keys = d.keys.get(i).cloned().unwrap_or_default();
            let f = ui.key_field(c, vl, &keys, listening, 8.5, State::focused(focus));
            c.entity(f).insert(SettingsAction::Bind(i));
            if !listening {
                let clr = ui.button(c, vl, Kind::Ghost, "Clear", None, 1.6, State::NONE);
                c.entity(clr).insert(SettingsAction::Clear(i));
            }
            Some(row)
        }
        Row::Setting { s: set, note } => {
            let v = s.values.get(&set.cvar).cloned().unwrap_or_default();
            let changed = s.pending.contains_key(&set.cvar);
            let st = State { focus, changed, ..State::NONE };
            let (row, vl) = ui.row(c, list, &set.label, *note, t.layout(L, "row_h"), st);
            c.entity(row).insert((SettingsAction::Row(i), crate::ui_cues::CueItem(format!("settings:{}:{i}", s.tab))));
            let vst = State::focused(focus);
            match (widget(set), &set.pick) {
                (Widget::Slider { min, max, .. }, _) => {
                    // A typed value past the range keeps its number; the
                    // thumb stops at the end.
                    let f = v.trim().parse::<f32>().map_or(0.0, |x| controls::frac_of(x, min, max));
                    let edit = s.edit.as_ref().filter(|e| e.row == i);
                    let typing = edit.map(|e| controls::Typing {
                        text: &e.text,
                        caret: e.text.char_indices().nth(e.caret).map_or(e.text.len(), |(b, _)| b),
                        anchor: e.text.char_indices().nth(e.anchor).map_or(e.text.len(), |(b, _)| b),
                        error: e.error.is_some(),
                        suffix: set.percent.then_some("%"),
                    });
                    let p = ui.slider_ex(c, vl, f, None, &set.shown(&v), 15.0, vst, typing);
                    c.entity(p.track).insert(SettingsAction::Slide(i));
                    c.entity(p.value).insert(SettingsAction::EditValue(i));
                    if let Some(why) = edit.and_then(|e| e.error.as_deref()) {
                        // Refused: why, beside the box (threat as a mark).
                        let n = ui.boxed(c, vl, UiStyle { gap: Some(0.4), margin: Some([0.0, 0.0, 0.0, 0.6].into()), ..UiStyle::row() });
                        ui.mark(c, n, Tone::Threat, 0.36, 0.36);
                        ui.sentence(c, n, NOTE, Tone::Fg2, why);
                    }
                }
                (Widget::Toggle, Pick::Choices(ch)) => {
                    let on = ch.get(1).is_some_and(|(cv, _)| host::same_value(cv, &v));
                    let slide = s.flip.filter(|(r, _)| *r == i).map(|(_, since)| controls::ToggleSlide { since, now: d.now, reduced: d.reduced });
                    let tg = ui.toggle_ex(c, vl, on, vst, slide);
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
                        c.entity(e).insert((SettingsAction::Pick { row: i, option: k }, crate::ui_cues::CueItem(format!("settings:list:{i}:{k}"))));
                    }
                }
                _ => {
                    let (minus, plus) = ui.stepper(c, vl, &set.shown(&v), 7.0, vst);
                    c.entity(minus).insert(SettingsAction::Step { row: i, dir: -1 });
                    c.entity(plus).insert(SettingsAction::Step { row: i, dir: 1 });
                }
            }
            Some(row)
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
    ui.sentence(c, big, SENTENCE, Tone::Fg2, "s, then it goes back");
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let k = ui.button(c, acts, Kind::Primary, "Keep", Some("Enter"), 2.4, State::focused(true));
    c.entity(k).insert(SettingsAction::Keep);
    let r = ui.button(c, acts, Kind::Secondary, "Revert", Some("Esc"), 2.4, State::NONE);
    c.entity(r).insert(SettingsAction::KeepRevert);
}

/// "Apply your changes?" on leaving: Apply / Discard / Stay.
fn leave_modal(c: &mut Commands, ui: &Ui, root: Entity, at: usize, changes: usize) {
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
    let m = ui.modal(c, shade, 32.0, false);
    ui.text(c, m, Role::DisplayBold, 1.35, Tone::Fg, "Apply your changes?");
    let line = if changes == 1 { "1 change is not applied yet.".to_string() } else { format!("{changes} changes are not applied yet.") };
    let tx = ui.boxed(c, m, UiStyle { w_pct: Some(100.0), margin: Some([0.8, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
    ui.paragraph(c, tx, Role::Text, SENTENCE, Tone::Fg2, &format!("{line} Apply keeps them and saves; Discard puts them back."));
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let buttons = [(Kind::Primary, "Apply"), (Kind::Secondary, "Discard"), (Kind::Ghost, "Stay")];
    for (k, ((kind, label), action)) in buttons.into_iter().zip(LEAVE_CHOICES).enumerate() {
        // Enter presses the focused one (←→ move it), Esc stays.
        let key = if k == at { Some("Enter") } else if action == SettingsAction::LeaveStay { Some("Esc") } else { None };
        let b = ui.button(c, acts, kind, label, key, 2.4, State::focused(k == at));
        c.entity(b).insert(action);
    }
}

/// A bind listening takes the mouse's buttons as binds: right-click isn't
/// Back then (`chimera_ui::shell::BackGuard`).
fn hold_back(s: Res<Settings>, guard: Option<ResMut<chimera_ui::shell::BackGuard>>) {
    if s.capture.is_some()
        && let Some(mut g) = guard
    {
        g.hold();
    }
}
