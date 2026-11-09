//! Drawing the open dialog: its frame, tabs, controls where the install's
//! layout puts them (greyed where Chimera lacks them), columns without it;
//! the player list's list; the Voice tab's microphone meter (`MicMeter`,
//! `MicMeter2`: the dark bars' picture, the lit one cut to the bars the
//! microphone's level lights, `mic_meter`, `meter_bars`).
//!
//! Known shortcuts (docs/tech-debt.md): Game menu drawing
//! (`game_menu::draw`): the microphone meter's scale (decibels from
//! `METER_FLOOR_DB`, -48) and fall (`METER_FALL`) are ours (CS:S's meter
//! is code: capture C-voice).

use super::*;

#[derive(Component)]
pub struct MenuRoot;

/// The menu's own layer (its backdrop, title and entries): over the HUD
/// and every in-game panel (the message of the day, team, class, buy,
/// radio and spectator menus, the scoreboard: `widgets::GAME_UI_Z`), under
/// every dialog's frame (`widgets::FRAME_Z` and up).
pub const MENU_Z: i32 = widgets::GAME_UI_Z;

/// What a page is drawn with: the root (open lists' outside clicks), the
/// look, the menu and its rows.
pub(super) struct Ctx<'a> {
    pub(super) root: Entity,
    pub(super) look: &'a Look<'a>,
    pub(super) menu: &'a GameMenu,
    pub(super) rows: &'a [Row],
    /// The options' pictures by name (`MenuUi::pictures`).
    pub(super) pictures: &'a HashMap<String, Handle<Image>>,
}

/// A dialog's size in scheme pixels: the layout's frame when it has one.
pub(super) fn dialog_size(menu: &GameMenu, page: Page) -> (f32, f32) {
    match page {
        // CS:S's options and Create Server dialogs: their pages
        // (`OptionsSubMultiplayer.res`: 496 x 314; Create Server's: 332 x
        // 364) in a property sheet, the buttons under it.
        Page::Settings => (512.0, 406.0),
        Page::NewGame => (348.0, 460.0),
        // The layout reaches to 264 x 124.
        Page::KeyboardAdvanced => (280.0, 134.0),
        Page::VideoAdvanced => menu
            .ui
            .0
            .as_ref()
            .and_then(|u| u.options.get("video_advanced"))
            .and_then(|l| l.get("OptionsSubVideoAdvancedDlg"))
            .map_or((482.0, 358.0), |c| (c.wide, c.tall)),
        Page::Gamma => menu
            .ui
            .0
            .as_ref()
            .and_then(|u| u.options.get("video_gamma"))
            .and_then(|l| l.get("OptionsSubVideoGammaDlg"))
            .map_or((290.0, 396.0), |c| (c.wide, c.tall)),
        Page::MultiplayerAdvanced => menu
            .ui
            .0
            .as_ref()
            .and_then(|u| u.options.get("multiplayer_advanced"))
            .and_then(|l| l.get("MultiplayerAdvancedDialog"))
            .map_or((540.0, 376.0), |c| (c.wide, c.tall)),
        Page::Extras | Page::Debug => {
            let n = menu.rows().len() as f32;
            (420.0, 44.0 + n * 28.0 + 12.0)
        }
        Page::PlayerList => menu
            .ui
            .0
            .as_ref()
            .and_then(|u| u.options.get("player_list"))
            .and_then(|l| l.get("PlayerListDialog"))
            .map_or((467.0, 388.0), |c| (c.wide, c.tall)),
        Page::Vote => vote_boxes(menu).size,
        _ => (360.0, 230.0),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    menu: Res<GameMenu>,
    fonts: Res<UiFonts>,
    menu_ui: Option<Res<MenuUi>>,
    hud: Option<Res<crate::map::hud::ActiveHud>>,
    details: Res<LoadingDetails>,
    shown: Query<Entity, With<MenuRoot>>,
    windows_q: Query<&Window>,
    mut windows: ResMut<Windows>,
    frame_backing: Option<ResMut<widgets::FrameBacking>>,
    mut last: Local<(Vec2, Option<Page>)>,
    version: Option<Res<crate::core::BuildVersion>>,
    reporter: Option<Res<crate::map::loading::LoadReporter>>,
    mut commands: Commands,
) {
    let size = windows_q
        .iter()
        .next()
        .map_or(Vec2::new(640.0, 480.0), |w| Vec2::new(w.width(), w.height()));
    let resized = (size - last.0).abs().max_element() > 0.5;
    if !menu.is_changed() && !resized && !hud.as_ref().is_some_and(|h| h.is_changed()) && !details.is_changed() {
        return;
    }
    // A dialog opened (or another in its place): in front.
    let page = (menu.open && menu.page != Page::Main).then_some(menu.page);
    if page != last.1
        && let Some(p) = page
    {
        windows.raise(p.window());
    }
    *last = (size, page);
    for e in &shown {
        commands.entity(e).despawn();
    }
    // Dialogs hide the menu under them: drawn over the main menu's
    // picture, or in a game over the world drawn again, dimmed
    // (`world_behind_dialogs`; black until its picture is ready).
    let backing = match (menu.open, menu.in_game) {
        (false, _) => None,
        (true, false) => menu_ui
            .as_ref()
            .and_then(|u| u.background_sized(size))
            .map(|(image, image_size)| widgets::Backing::Picture(image, image_size))
            .or(Some(widgets::Backing::Color(Color::BLACK))),
        (true, true) => Some(widgets::Backing::World(Color::BLACK)),
    };
    if let Some(mut b) = frame_backing {
        b.set_if_neq(widgets::FrameBacking(backing));
    }
    if !menu.open {
        return;
    }
    let accent = hud
        .as_ref()
        .and_then(|h| h.0.color("FgColor"))
        .map(|c| c.with_alpha(1.0))
        .unwrap_or(Color::srgb_u8(255, 176, 0));
    let look = Look {
        ui: menu.ui.0.as_deref(),
        fonts: &fonts,
        s: (size.y / 720.0).clamp(0.6, 3.0),
        height: size.y,
        accent,
    };
    // In a game the game shows through, darkened (GameUI's backdrop); at
    // the main menu the game's background picture covers the screen (or
    // black without it).
    let backdrop = if !menu.in_game {
        Color::BLACK
    } else if look.has_scheme() {
        look.color("MainMenu.Backdrop", [0, 0, 0, 156])
    } else {
        Color::srgba(0.0, 0.0, 0.0, 0.35)
    };
    let root = commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(backdrop),
            // Over the HUD and the in-game panels, under the frames
            // (`widgets::FRAME_Z`) and the console; a click on it stops
            // here, so the panels under it take none while it is open.
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(MENU_Z),
        ))
        .id();
    if !menu.in_game
        && let Some(image) = menu_ui.as_ref().and_then(|u| u.background(size))
    {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            ImageNode {
                image,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            ChildOf(root),
        ));
    }
    let title_font = menu_ui.as_ref().and_then(|u| u.title_font.clone());
    if let Some(failure) = &menu.failure {
        failure_frame(&mut commands, root, &menu, &look, failure);
        return;
    }
    if let Some(map) = &menu.loading {
        loading_frame(&mut commands, root, &menu, &look, map, reporter.as_ref().and_then(|r| r.current()));
        if details.0 != 0 {
            details_panel(&mut commands, root, &look, size);
        }
        return;
    }
    let version = version.map_or("", |v| v.0);
    main_list(&mut commands, root, &menu, &look, size, title_font, version);
    if menu.page == Page::Main {
        return;
    }
    let rows = menu.rows();
    let no_pictures = HashMap::new();
    let pictures = menu_ui.as_ref().map_or(&no_pictures, |u| &u.pictures);
    let ctx = Ctx {
        root,
        look: &look,
        menu: &menu,
        rows: &rows,
        pictures,
    };
    // An Advanced dialog shows over the options.
    if menu.page.over_options() {
        let mut under = menu.clone();
        under.page = Page::Settings;
        under.combo = None;
        under.focus = usize::MAX;
        let under_rows = under.rows();
        let c = Ctx {
            root,
            look: &look,
            menu: &under,
            rows: &under_rows,
            pictures,
        };
        dialog(&mut commands, &c, Page::Settings);
    }
    dialog(&mut commands, &ctx, menu.page);
}

/// One of the menu's dialogs in its frame.
pub(super) fn dialog(commands: &mut Commands, ctx: &Ctx, page: Page) {
    let (look, menu) = (ctx.look, ctx.menu);
    let (w, h) = dialog_size(menu, page);
    let title = match page {
        Page::Main => String::new(),
        Page::NewGame => menu.text("#GameUI_CreateServer", "Create Server"),
        Page::Bots => "Bots".into(),
        Page::Team => "Choose a Team".into(),
        Page::Settings => menu.text("#GameUI_Options", "Options"),
        Page::KeyboardAdvanced => menu.text("#GameUI_KeyboardAdvanced_Title", "Keyboard - Advanced"),
        Page::VideoAdvanced => menu.text("#GameUI_VideoAdvanced_Title", "Video - Advanced"),
        Page::Gamma => menu.text("#GameUI_AdjustGamma_Title", "Adjust brightness levels"),
        Page::MultiplayerAdvanced => menu.text("#GameUI_MultiplayerAdvanced", "Multiplayer Advanced"),
        Page::Extras => "Lucker Party Options".into(),
        Page::Debug => "Chimera Debug Options".into(),
        Page::PlayerList => menu.player_list_title(),
        // Ours (the file gives none).
        Page::Vote => "Call Vote".into(),
    };
    let mut spec = FrameSpec::new(page.window());
    if page.over_options() {
        spec = spec.modal();
    }
    let parts = widgets::frame(commands, ctx.root, look, spec, (w, h), &title);
    let frame = parts.frame;
    if let Some(close) = parts.close {
        commands.entity(close).insert(Hit(Target::Close, 0));
    }
    match page {
        Page::Settings => {
            let names: Vec<(String, bool)> = TABS.iter().map(|(_, t, o)| (menu.text(t, o), true)).collect();
            // Each as wide as its words (VGUI's tabs), kept on the sheet.
            widgets::tabs(commands, frame, look, (8.0, 30.0), &names, menu.tab.index(), None, Some(w - 16.0), |i| {
                Hit(Target::Tab(i), 0)
            });
            let content = sheet_page(commands, frame, look, (8.0, 57.0, w - 16.0, h - 57.0 - 36.0));
            if menu.tab == Tab::Keyboard {
                keyboard_tab(commands, content, look, menu, ctx.rows);
            } else {
                page_controls(commands, ctx, content, (w - 16.0, h - 57.0 - 36.0));
            }
            dialog_buttons(commands, ctx, frame, (w, h), &[Action::Ok, Action::Cancel, Action::Apply]);
        }
        Page::NewGame => {
            let names: Vec<(String, bool)> = menu.create_tabs().iter().map(|(t, o)| (menu.text(t, o), true)).collect();
            widgets::tabs(commands, frame, look, (8.0, 30.0), &names, menu.create_tab, None, Some(w - 16.0), |i| {
                Hit(Target::Tab(i), 0)
            });
            let size = (w - 16.0, h - 57.0 - 38.0);
            let content = sheet_page(commands, frame, look, (8.0, 57.0, size.0, size.1));
            if menu.create_tab == 1 {
                game_options(commands, ctx, content, size);
            } else {
                page_controls(commands, ctx, content, size);
            }
            dialog_buttons(commands, ctx, frame, (w, h), &[Action::Start, Action::Cancel]);
        }
        Page::KeyboardAdvanced | Page::VideoAdvanced | Page::Gamma => {
            page_controls(commands, ctx, frame, (w, h));
        }
        Page::MultiplayerAdvanced => {
            // Its list where the layout puts it (`CPanelListPanel`), the
            // buttons from the layout too.
            let at = menu
                .layout()
                .and_then(|l| l.get("PanelListPanel"))
                .map(|c| (coord(c.x), coord(c.y), c.wide, c.tall));
            options_list(commands, ctx, frame, at.unwrap_or((16.0, 56.0, w - 60.0, h - 102.0)));
            if menu.layout().is_some() {
                page_controls(commands, ctx, frame, (w, h));
            } else {
                dialog_buttons(commands, ctx, frame, (w, h), &[Action::Ok, Action::Cancel]);
            }
        }
        Page::Extras | Page::Debug => {
            column(commands, ctx, frame, (16.0, 36.0, w - 32.0), true);
            dialog_buttons(commands, ctx, frame, (w, h), &[Action::Ok, Action::Cancel]);
        }
        Page::PlayerList => {
            let at = menu
                .layout()
                .and_then(|l| l.get("PlayerList"))
                .map(|c| (coord(c.x), coord(c.y), c.wide, c.tall));
            player_list(commands, ctx, frame, at.unwrap_or((28.0, 38.0, w - 57.0, h - 109.0)));
            if menu.layout().is_some() {
                page_controls(commands, ctx, frame, (w, h));
            } else {
                // Mute and Close under the list.
                let n = ctx.rows.len();
                for (k, i) in [n - 2, n - 1].into_iter().enumerate() {
                    if let Row::Button { label: text, enabled, .. } = &ctx.rows[i] {
                        let state = Btn::enabled(*enabled).focused(menu.focus == i);
                        let rect = (28.0 + k as f32 * 170.0, h - 62.0, 160.0, 24.0);
                        widgets::button(commands, frame, look, rect, text, state, 0, Hit(Target::Row(i), 0));
                    }
                }
            }
        }
        Page::Vote => vote_dialog(commands, ctx, frame),
        _ => column(commands, ctx, frame, (16.0, 36.0, w - 32.0), false),
    }
}

/// The Voice tab's meter's lit bars' picture (`OptionsSubVoice.res`'s
/// `MicMeter2`).
pub(super) const METER_LIT: &str = "resource/mic_meter_live";
/// The Voice tab's meter's pictures (dark and lit bars).
pub(super) const METER: &str = "resource/mic_meter";

/// The Voice tab's meter's lit bars: `bars` of them, `step` window pixels
/// apart; `mic_meter` shows as many as the microphone's level.
#[derive(Component)]
pub(super) struct MicMeter {
    pub(super) bars: u32,
    pub(super) step: f32,
}

/// The meter's bars lit for a microphone level (0 to 1, a frame's loudest
/// sample): on a decibel scale from `METER_FLOOR_DB` (none) to full scale
/// (all).
pub fn meter_bars(level: f32, bars: u32) -> u32 {
    if level <= 0.0 {
        return 0;
    }
    let db = 20.0 * level.log10();
    let f = ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0);
    (f * bars as f32).round() as u32
}

/// The meter's quietest level shown (dB below full scale; ours).
pub const METER_FLOOR_DB: f32 = -48.0;

/// The meter follows the microphone: a new peak at once, falling by
/// `METER_FALL` a second (ours).
pub(super) fn mic_meter(
    level: Option<Res<crate::client::voice::VoiceLevel>>,
    time: Res<Time<Real>>,
    mut shown: Local<f32>,
    mut meters: Query<(&mut Node, &MicMeter)>,
) {
    let now = level.map_or(0.0, |l| l.0);
    *shown = now.max(*shown * (-METER_FALL * time.delta_secs()).exp());
    for (mut node, m) in &mut meters {
        let w = px(meter_bars(*shown, m.bars) as f32 * m.step);
        if node.width != w {
            node.width = w;
        }
    }
}

/// How fast the meter falls (its level times e^-METER_FALL a second).
const METER_FALL: f32 = 6.0;

/// Controls of the game's layouts left out on purpose (a deliberate
/// difference, docs/plans/active/ui-parity.md): Virtual Reality Mode and
/// its label (Chimera won't support VR).
pub const LEFT_OUT: [&str; 2] = ["VRMode", "VRModeLabel"];

/// A property sheet's page: a raised box under its tabs.
pub(super) fn sheet_page(commands: &mut Commands, frame: Entity, look: &Look, (x, y, w, h): (f32, f32, f32, f32)) -> Entity {
    commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, x, y, w, h)
            },
            bevel(look, true),
            ChildOf(frame),
        ))
        .id()
}

/// A dialog's buttons at its bottom right (OK, Cancel, Apply; Start,
/// Cancel), the default one ringed.
pub(super) fn dialog_buttons(commands: &mut Commands, ctx: &Ctx, frame: Entity, (w, h): (f32, f32), actions: &[Action]) {
    let default = ctx.menu.default_action();
    let mut x = w - 8.0 - (72.0 + 6.0) * actions.len() as f32 + 6.0;
    for action in actions {
        if let Some((i, Row::Button { label, enabled, .. })) = ctx
            .rows
            .iter()
            .enumerate()
            .find(|(_, r)| matches!(r, Row::Button { action: a, .. } if a == action))
        {
            let state = Btn::enabled(*enabled)
                .focused(ctx.menu.focus == i)
                .default_button(default.as_ref() == Some(action));
            widgets::button(commands, frame, ctx.look, (x, h - 8.0 - 24.0, 72.0, 24.0), label, state, 0, Hit(Target::Row(i), 0));
        }
        x += 78.0;
    }
}

pub(super) fn label_of(row: &Row) -> String {
    match row {
        Row::Button { label, .. } | Row::Control { label, .. } | Row::Bind { label, .. } => label.clone(),
        Row::Heading(t) | Row::Info(t) => t.clone(),
        Row::Greyed { label, .. } => label.clone(),
        Row::Player { name, .. } => name.clone(),
    }
}

/// A row's control at a box (scheme pixels in `parent`): a combo box (its
/// list when open), check box, radio button, slider (its Low / High words
/// under it when the layout gives them), text entry. `text`: a check
/// box's words (the layout's), else none.
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    commands: &mut Commands,
    ctx: &Ctx,
    parent: Entity,
    i: usize,
    row: &Row,
    rect: (f32, f32, f32, f32),
    text: Option<&str>,
    layout: Option<&UiControl>,
) {
    let (look, menu) = (ctx.look, ctx.menu);
    let Row::Control { label: name, control, .. } = row else {
        return;
    };
    let focused = menu.focus == i;
    let (x, y, w, h) = rect;
    match control {
        Control::Combo { entries, text: shown, .. } => {
            let open = menu.combo.filter(|c| c.owner == i);
            let ch = h.min(24.0);
            widgets::combo_box(
                commands,
                parent,
                look,
                (x, y, w, ch),
                shown,
                true,
                open.is_some(),
                focused,
                (Hit(Target::Row(i), 0), WheelCombo(i), RelativeCursorPosition::default()),
            );
            if let Some(list) = open {
                widgets::combo_popup(
                    commands,
                    parent,
                    ctx.root,
                    look,
                    (x, y, w, ch),
                    entries,
                    &list,
                    |k| Hit(Target::ComboItem(k), 0),
                    Hit(Target::Outside, 0),
                );
            }
        }
        Control::Check(on) => {
            widgets::check_button(
                commands,
                parent,
                look,
                rect,
                text.unwrap_or(""),
                *on,
                true,
                focused,
                Hit(Target::Row(i), 0),
            );
        }
        Control::Radio(on) => {
            widgets::radio_button(
                commands,
                parent,
                look,
                rect,
                text.unwrap_or(name),
                *on,
                true,
                focused,
                Hit(Target::Row(i), 0),
            );
        }
        Control::Slider { fraction, .. } => {
            let track_h = 24.0;
            widgets::slider(
                commands,
                parent,
                look,
                (x, y, w, track_h),
                *fraction,
                11,
                true,
                SliderTrack {
                    owner: SliderOwner::Menu,
                    id: i,
                    inset: 0.0,
                },
            );
            if focused {
                commands.spawn((
                    place(look, x, y, w, track_h),
                    Outline::new(px(1.0), px(0.0), look.dark()),
                    ChildOf(parent),
                ));
            }
            // VGUI's slider words under its ends (`leftText`, `rightText`).
            let words = |key: &str| {
                layout
                    .and_then(|c| c.keys.get(key))
                    .map(|t| match t.strip_prefix('#') {
                        Some(_) => menu.text(t, ""),
                        None => t.clone(),
                    })
                    .filter(|t| !t.is_empty() && t.parse::<f32>().is_err())
            };
            if h >= 36.0 {
                let font = look.font("DefaultSmall", (13.0, false));
                if let Some(t) = words("lefttext") {
                    label(commands, parent, look, (x, y + track_h, w / 2.0, 14.0), &t, font.clone(), look.text(), -1);
                }
                if let Some(t) = words("righttext") {
                    label(commands, parent, look, (x + w / 2.0, y + track_h, w / 2.0, 14.0), &t, font, look.text(), 1);
                }
            }
        }
        Control::Text { text: value, .. } => {
            let caret = focused.then_some(menu.caret);
            widgets::text_entry(
                commands,
                parent,
                look,
                (x, y, w, h.min(24.0)),
                value,
                caret,
                false,
                true,
                (
                    Hit(Target::Row(i), 0),
                    TextHit {
                        row: i,
                        shown: value.clone(),
                        width: w,
                    },
                ),
            );
        }
    }
}

/// Whether a layout control is a page or frame part, not a control drawn
/// on it.
pub(super) fn frame_part(c: &UiControl, (w, h): (f32, f32)) -> bool {
    let class = class_of(c);
    let x = coord(c.x);
    let y = coord(c.y);
    matches!(c.kind, UiKind::Frame)
        || matches!(class, "BuildModeDialog" | "Menu" | "FrameSystemButton" | "CPanelListPanel")
        || c.name.starts_with("frame_")
        || (c.wide >= w - 24.0 && c.tall >= h - 64.0)
        || x >= w
        || y >= h
}

/// The open page's controls where the game's layout puts them: those
/// Chimera has as working controls, the rest greyed (labels as they are);
/// without the layout, a column.
pub(super) fn page_controls(commands: &mut Commands, ctx: &Ctx, parent: Entity, (w, h): (f32, f32)) {
    let (look, menu) = (ctx.look, ctx.menu);
    let Some(layout) = menu.layout() else {
        column(commands, ctx, parent, (20.0, 14.0, w - 40.0), menu.page.over_options());
        return;
    };
    let rects = layout.rects(Rect::new(0.0, 0.0, w, h), 1.0);
    let font = look.default_font();
    // Each row's control, by its field's name.
    let row_at = |name: &str| {
        ctx.rows.iter().position(|r| match r {
            Row::Control { field, .. } => menu.field_name(*field).is_some_and(|f| f.eq_ignore_ascii_case(name)),
            Row::Button {
                action: Action::Ok, ..
            } if menu.page == Page::MultiplayerAdvanced => name.eq_ignore_ascii_case("OK"),
            Row::Button {
                action: Action::Cancel,
                ..
            } if menu.page == Page::MultiplayerAdvanced => name.eq_ignore_ascii_case("Cancel"),
            // The brightness dialog names them OKButton and Button1.
            Row::Button {
                action: Action::Ok, ..
            } if menu.page == Page::Gamma => name.eq_ignore_ascii_case("OKButton"),
            Row::Button {
                action: Action::Cancel,
                ..
            } if menu.page == Page::Gamma => name == "Button1",
            Row::Button {
                action: Action::Ok, ..
            } => name == "Button1" && menu.page.over_options(),
            Row::Button {
                action: Action::Cancel,
                ..
            } => name == "Button2" && menu.page.over_options(),
            // The player list's (`PlayerListDialog.res`).
            Row::Button {
                action: Action::Run(..),
                ..
            } if menu.page == Page::PlayerList => name.eq_ignore_ascii_case("MuteButton"),
            Row::Button {
                action: Action::Back, ..
            } if menu.page == Page::PlayerList => name == "Button1",
            // The Voice tab's Test Microphone.
            Row::Button {
                action: Action::Run(line, _),
                ..
            } if line.starts_with("voice_test") => name == TEST_MICROPHONE,
            Row::Button { action, .. } => advanced_button(action).is_some_and(|b| b == name),
            _ => false,
        })
    };
    let drawn: Vec<Rect> = layout
        .controls
        .iter()
        .zip(&rects)
        .filter(|(c, _)| row_at(&c.name).is_some())
        .map(|(_, r)| *r)
        .collect();
    // Controls whose label the install has no words for (VR mode): left
    // out with it, as the game hides what it doesn't offer.
    let unworded: Vec<&str> = layout
        .controls
        .iter()
        .filter(|c| c.text.starts_with('#'))
        .filter_map(|c| c.keys.get("associate").map(String::as_str))
        .collect();
    for (c, r) in layout.controls.iter().zip(&rects) {
        let rect = (r.min.x, r.min.y, r.width(), r.height());
        let row = row_at(&c.name);
        if frame_part(c, (w, h)) || (!c.visible && row.is_none()) {
            continue;
        }
        if row.is_none() && unworded.iter().any(|n| n.eq_ignore_ascii_case(&c.name)) {
            continue;
        }
        if LEFT_OUT.iter().any(|n| n.eq_ignore_ascii_case(&c.name)) {
            continue;
        }
        let class = class_of(c);
        if let Some(i) = row {
            match &ctx.rows[i] {
                Row::Button { label: text, enabled, action } => {
                    let state = Btn::enabled(*enabled)
                        .focused(menu.focus == i)
                        .default_button(matches!(action, Action::Ok));
                    widgets::button(commands, parent, look, rect, text, state, 0, Hit(Target::Row(i), 0));
                }
                row => {
                    let text = (!c.text.is_empty() && !c.text.starts_with('#')).then_some(c.text.as_str());
                    control(commands, ctx, parent, i, row, rect, text, Some(c));
                }
            }
            continue;
        }
        let text = if c.text.starts_with('#') { "" } else { c.text.as_str() };
        let image_name = c.keys.get("image").map(|n| n.to_lowercase()).unwrap_or_default();
        let picture = ctx.pictures.get(&image_name);
        match class.to_ascii_lowercase().as_str() {
            "label" => {
                // Not over a control we draw (the video Advanced dialog's
                // note sits where its HDR box is).
                let covered = |d: &Rect| {
                    let i = d.intersect(*r);
                    !i.is_empty() && i.width() * i.height() > 0.25 * r.width() * r.height()
                };
                if text.is_empty() || drawn.iter().any(covered) {
                    continue;
                }
                let color = if c.dull { look.dull() } else { look.text() };
                let f = c.font.as_deref().map_or_else(|| font.clone(), |n| look.font(n, (13.0, false)));
                let align = match c.align {
                    crate::map::hud::UiAlign::East | crate::map::hud::UiAlign::NorthEast | crate::map::hud::UiAlign::SouthEast => 1,
                    crate::map::hud::UiAlign::Center | crate::map::hud::UiAlign::North | crate::map::hud::UiAlign::South => 0,
                    _ => -1,
                };
                if c.wrap || r.height() > 30.0 {
                    let e = commands.spawn((place(look, rect.0, rect.1, rect.2, rect.3), ChildOf(parent))).id();
                    commands.spawn((
                        Text::new(text),
                        f,
                        TextColor(color),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        ChildOf(e),
                    ));
                } else {
                    let e = label(commands, parent, look, rect, text, f, color, align);
                    // Words wider than their box in our stand-in faces run over,
                    // not cut.
                    commands.entity(e).entry::<Node>().and_modify(|mut n| n.overflow = Overflow::visible());
                }
            }
            "divider" => {
                commands.spawn((
                    Node {
                        border: UiRect::all(px(1.0)),
                        ..place(look, rect.0, rect.1, rect.2, 2.0)
                    },
                    bevel(look, false),
                    ChildOf(parent),
                ));
            }
            "button" | "urlbutton" | "togglebutton" => {
                if !text.is_empty() {
                    widgets::button(commands, parent, look, rect, text, Btn::enabled(false), 0, ());
                }
            }
            // Without words (tokens this install lacks) a greyed box says
            // nothing: left out.
            "checkbutton" | "ccvartogglecheckbutton" | "ccvarnegatecheckbutton" if !text.is_empty() => {
                widgets::check_button(commands, parent, look, rect, text, false, false, false, ());
            }
            "radiobutton" if !text.is_empty() => {
                widgets::radio_button(commands, parent, look, rect, text, false, false, false, ());
            }
            "combobox" | "clabeledcommandcombobox" => {
                widgets::combo_box(commands, parent, look, (rect.0, rect.1, rect.2, rect.3.min(24.0)), "", false, false, false, ());
            }
            "ccvarslider" | "slider" => {
                widgets::slider(
                    commands,
                    parent,
                    look,
                    (rect.0, rect.1, rect.2, 24.0),
                    0.0,
                    11,
                    false,
                    SliderTrack {
                        owner: SliderOwner::Menu,
                        id: usize::MAX,
                        inset: 0.0,
                    },
                );
            }
            "textentry" => {
                widgets::text_entry(commands, parent, look, (rect.0, rect.1, rect.2, rect.3.min(24.0)), "", None, false, false, ());
            }
            "crosshairimagepanelcs" => crosshair_preview(commands, parent, look, menu, rect),
            "imagepanel" if picture.is_some() && image_name.starts_with(METER) => {
                // The Voice tab's meter (`MicMeter`, `MicMeter2`): its
                // pictures at their own size (VGUI doesn't stretch them; the
                // bars fill the panel's left), the dark bars, then as many
                // lit ones as the microphone's level (`mic_meter`).
                let lit = image_name == METER_LIT;
                let bars = c.keys.get("barcount").and_then(|n| n.parse().ok()).unwrap_or(19);
                let spacing = c.keys.get("barspacing").and_then(|n| n.parse().ok()).unwrap_or(8.0);
                let size = menu
                    .ui
                    .0
                    .as_ref()
                    .and_then(|u| u.option_images.get(&image_name))
                    .map_or((rect.2, rect.3), |p| (p.width as f32, p.height as f32));
                let mut clip = commands.spawn((
                    Node {
                        overflow: Overflow::clip(),
                        ..place(look, rect.0, rect.1, rect.2, rect.3)
                    },
                    ChildOf(parent),
                ));
                if lit {
                    clip.insert(MicMeter {
                        bars,
                        step: spacing * look.s,
                    })
                    .entry::<Node>()
                    .and_modify(|mut n| n.width = px(0.0));
                }
                let clip = clip.id();
                commands.spawn((
                    place(look, 0.0, 0.0, size.0, size.1),
                    ImageNode {
                        image: picture.cloned().unwrap_or_default(),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                    ChildOf(clip),
                ));
            }
            "imagepanel" if picture.is_some() => {
                // Its picture over its box, as the brightness dialog's
                // test lines.
                commands.spawn((
                    place(look, rect.0, rect.1, rect.2, rect.3),
                    ImageNode {
                        image: picture.cloned().unwrap_or_default(),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                    ChildOf(parent),
                ));
            }
            "imagepanel" => {
                commands.spawn((
                    Node {
                        border: UiRect::all(px(1.0)),
                        ..place(look, rect.0, rect.1, rect.2, rect.3)
                    },
                    bevel(look, false),
                    BackgroundColor(look.sunken_bg()),
                    ChildOf(parent),
                ));
            }
            _ => {}
        }
    }
}

/// Rows of labelled controls in a column at (x, y), `w` wide: ours (Lucker
/// Party Options), the Bots and Team pages, and pages without the
/// install's layout; `buttons`: the dialog draws its own buttons.
pub(super) fn column(commands: &mut Commands, ctx: &Ctx, parent: Entity, (x, y, w): (f32, f32, f32), buttons: bool) {
    let (look, menu) = (ctx.look, ctx.menu);
    let row_h = 28.0;
    let font = look.font("Default", (16.0, false));
    let label_w = (w * 0.45).round();
    let control_w = (w - label_w).min(220.0);
    let mut ry = y;
    for (i, row) in ctx.rows.iter().enumerate() {
        match row {
            Row::Info(t) | Row::Heading(t) | Row::Greyed { label: t, .. } => {
                label(commands, parent, look, (x, ry, w, row_h - 4.0), t, font.clone(), look.dull(), -1);
            }
            Row::Button { label: text, enabled, action } => {
                if buttons && matches!(action, Action::Ok | Action::Cancel | Action::Apply | Action::Start) {
                    continue;
                }
                let state = Btn::enabled(*enabled).focused(menu.focus == i);
                widgets::button(commands, parent, look, (x, ry, w.min(260.0), 24.0), text, state, -1, Hit(Target::Row(i), 0));
            }
            Row::Bind { .. } | Row::Player { .. } => continue,
            Row::Control { label: text, control: c, .. } => {
                if matches!(c, Control::Check(_) | Control::Radio(_)) {
                    control(commands, ctx, parent, i, row, (x, ry, w, 24.0), Some(text), None);
                } else {
                    label(commands, parent, look, (x, ry, label_w, 24.0), text, font.clone(), look.text(), -1);
                    let slider = matches!(c, Control::Slider { .. });
                    let cw = if slider { control_w - 48.0 } else { control_w };
                    control(commands, ctx, parent, i, row, (x + label_w, ry, cw, 24.0), None, None);
                    if let Control::Slider { text: value, .. } = c {
                        label(commands, parent, look, (x + label_w + control_w - 44.0, ry, 44.0, 24.0), value, font.clone(), look.text(), 1);
                    }
                }
            }
        }
        ry += row_h;
    }
}

/// The player list's list (VGUI's `ListPanel`): its column headings
/// ("Player name", "Properties"), a row per player (the selected one lit),
/// a scroll bar.
pub(super) fn player_list(commands: &mut Commands, ctx: &Ctx, parent: Entity, (lx, ly, lw, lh): (f32, f32, f32, f32)) {
    let (look, menu) = (ctx.look, ctx.menu);
    let list = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                overflow: Overflow::clip(),
                ..place(look, lx, ly, lw, lh)
            },
            bevel(look, false),
            BackgroundColor(look.color("ListPanel.BgColor", [0, 0, 0, 128])),
            RelativeCursorPosition::default(),
            WheelList,
            ChildOf(parent),
        ))
        .id();
    let (len, shown) = menu.list();
    let bar_w = look.number("ScrollBar.Wide", 17.0);
    let row_h = ((lh - 22.0) / shown.max(1) as f32).floor();
    let width = lw - bar_w - 4.0;
    let props_x = (width * 0.6).round();
    let header = look.font("DefaultBold", (16.0, true));
    let font = look.default_font();
    let header_color = look.color("SectionedListPanel.HeaderTextColor", [255, 255, 255, 255]);
    let name_head = menu.text("#GameUI_PlayerName", "Player name");
    label(commands, list, look, (6.0, 1.0, props_x - 8.0, 20.0), &name_head, header.clone(), header_color, -1);
    let props_head = menu.text("#GameUI_Properties", "Properties");
    label(commands, list, look, (props_x, 1.0, width - props_x, 20.0), &props_head, header, header_color, -1);
    commands.spawn((
        place(look, 2.0, 21.0, width - 2.0, 1.0),
        BackgroundColor(look.color("SectionedListPanel.DividerColor", [0, 0, 0, 255])),
        ChildOf(list),
    ));
    let text = look.color("ListPanel.TextColor", [190, 190, 190, 255]);
    for (k, i) in (menu.scroll..(menu.scroll + shown).min(len)).enumerate() {
        let Row::Player { name, properties } = &ctx.rows[i] else { continue };
        let y = 22.0 + k as f32 * row_h;
        let selected = menu.key_row == Some(i);
        let (bg, fg) = if selected {
            (look.selected_bg(), look.selected_text())
        } else {
            (Color::NONE, text)
        };
        let e = commands
            .spawn((
                place(look, 1.0, y, width, row_h),
                BackgroundColor(bg),
                Hit(Target::Row(i), 0),
                widgets::clickable(),
                ChildOf(list),
            ))
            .id();
        label(commands, e, look, (5.0, 0.0, props_x - 8.0, row_h), name, font.clone(), fg, -1);
        label(commands, e, look, (props_x - 1.0, 0.0, width - props_x, row_h), properties, font.clone(), fg, -1);
    }
    scroll_bar(commands, list, look, (lw - bar_w - 2.0, 0.0, lh - 2.0), (len, shown, menu.scroll));
}

/// A scrolled list of labelled controls (VGUI's `CPanelListPanel`: Create
/// Server's Game page, Multiplayer > Advanced) at a box in `parent`: the
/// open list's rows (`GameMenu::list`), each its label and control (a
/// check box its own words), greyed rows drawn disabled.
pub(super) fn options_list(commands: &mut Commands, ctx: &Ctx, parent: Entity, (lx, ly, lw, lh): (f32, f32, f32, f32)) {
    let (look, menu) = (ctx.look, ctx.menu);
    let list = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                overflow: Overflow::clip(),
                ..place(look, lx, ly, lw, lh)
            },
            bevel(look, false),
            RelativeCursorPosition::default(),
            WheelList,
            ChildOf(parent),
        ))
        .id();
    let (len, shown) = menu.list();
    let bar_w = look.number("ScrollBar.Wide", 17.0);
    let row_h = ((lh - 4.0) / shown.max(1) as f32).floor();
    let font = look.default_font();
    let inner = lw - bar_w - 4.0;
    let label_w = (inner * 0.5).round();
    for (k, i) in (menu.scroll..(menu.scroll + shown).min(len)).enumerate() {
        let y = 2.0 + k as f32 * row_h;
        let row = &ctx.rows[i];
        match row {
            Row::Control { label: text, control: c, .. } => {
                if matches!(c, Control::Check(_)) {
                    control(commands, ctx, list, i, row, (4.0, y, inner - 4.0, 24.0), Some(text), None);
                } else {
                    label(commands, list, look, (6.0, y, label_w - 8.0, 24.0), text, font.clone(), look.text(), -1);
                    control(commands, ctx, list, i, row, (label_w, y, inner - label_w, 24.0), None, None);
                }
            }
            Row::Greyed { label: text, control: c } => {
                let rect = (label_w, y, inner - label_w, 24.0);
                match c {
                    Control::Check(on) => {
                        widgets::check_button(commands, list, look, (4.0, y, inner - 4.0, 24.0), text, *on, false, false, ());
                        continue;
                    }
                    Control::Combo { text: shown, .. } => {
                        widgets::combo_box(commands, list, look, rect, shown, false, false, false, ());
                    }
                    Control::Text { text: value, .. } => {
                        widgets::text_entry(commands, list, look, rect, value, None, false, false, ());
                    }
                    Control::Radio(_) | Control::Slider { .. } => {}
                }
                label(commands, list, look, (6.0, y, label_w - 8.0, 24.0), text, font.clone(), look.disabled(), -1);
            }
            _ => {}
        }
    }
    scroll_bar(commands, list, look, (lw - bar_w - 2.0, 0.0, lh - 2.0), (len, shown, menu.scroll));
}

/// A layout coordinate from its start edge (dialog layouts use plain
/// numbers).
pub(super) fn coord(c: crate::map::hud::HudCoord) -> f32 {
    match c {
        crate::map::hud::HudCoord::Start(v) | crate::map::hud::HudCoord::Centre(v) | crate::map::hud::HudCoord::End(v) => v,
    }
}

/// Whose scroll bar thumb (`widgets::ScrollThumb`): the open page's list.
pub(super) const MENU_SCROLL: widgets::WindowId = "menu";

/// The open list's scroll bar (`widgets::scroll_bar`): arrows and the
/// track scroll (`Target::Scroll`, by its step), the thumb drags
/// (`MENU_SCROLL`).
pub(super) fn scroll_bar(commands: &mut Commands, parent: Entity, look: &Look, rect: (f32, f32, f32), list: (usize, usize, usize)) {
    widgets::scroll_bar(commands, parent, look, rect, list, (MENU_SCROLL, 0), |step| Hit(Target::Scroll, step));
}
