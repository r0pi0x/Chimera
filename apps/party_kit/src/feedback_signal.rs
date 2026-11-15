//! The report dialog in the Signal look (feature `present`; a theme with
//! `[layout.esc]`, the Ultimate Gun Game's; docs/plans/active/
//! feedback-reports.md, P1b): draws `client::feedback::Feedback` over the
//! match (which goes on), in the kit's controls (`chimera_ui::controls`).
//!
//! A modal plate with the white top bar: "Report" and a segmented **Bug |
//! Feedback** (Tab or click); the text area ("What happened? What did you
//! expect?", the characters left, Enter a new line; Ctrl+A's selection
//! drawn on the accent tint); the screenshot's
//! thumbnail with the **Include screenshot** toggle; a folded **What's
//! sent** (`kv` lines: build, map, position, weapon, screenshot, the
//! rest) and the one-line privacy notice; where the report goes (the
//! outbox's `destination`); **Cancel** (Esc) and **Send** (Ctrl+Enter).
//! After Send a notice at the top for a few seconds: "Sending report...",
//! then "Report sent" or "Saved, sends when the service answers (N
//! waiting)" (`client::feedback`'s deliveries).
//!
//! Besides drawing: the key is held off while a bind listens for one
//! (Settings', the Controls page's); the Escape menu closes when the key
//! is pressed with it open (the screenshot shows the match); clicks on the
//! dialog's parts are `FeedbackAction`s. `ui_scene report` stages it with
//! made-up words (`stage`).

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::feedback::{Category, Feedback, FeedbackAction, FeedbackOutbox, MAX_TEXT, Phase};
use chimera_ui::{
    Role, Tone, UiStyle,
    controls::{Kind, State, Ui},
};

/// Over the Escape menu (90), under the console (100).
pub const DIALOG_Z: i32 = 95;

/// The plate's width, cq.
const WIDTH: f32 = 46.0;

/// The privacy notice.
pub const NOTICE: &str = "Sent to this playtest's host only. No passwords, tokens or game files.";

#[derive(Component)]
pub(crate) struct DialogRoot;

#[derive(Component)]
pub(crate) struct NoticeRoot;

pub struct FeedbackSignalPlugin;

impl Plugin for FeedbackSignalPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Feedback>() {
            app.add_plugins(chimera_client::client::feedback::FeedbackPlugin);
        }
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "report", scene_report);
        app.add_systems(
            Update,
            (flags, close_menus, clicks, show, show_notice)
                .chain()
                .before(chimera_client::client::feedback::FeedbackSystems)
                .after(crate::esc_menu::EscMenuSystems),
        );
    }
}

/// The dialog is ours under Signal; the key held off while a bind
/// listens for one.
fn flags(
    theme: Option<Res<chimera_ui::Theme>>,
    settings: Option<Res<crate::settings::Settings>>,
    controls: Option<Res<crate::controls::ControlsPage>>,
    mut feedback: ResMut<Feedback>,
) {
    let dialog = crate::esc_menu::signal(theme.as_deref());
    let blocked = settings.is_some_and(|s| s.capture.is_some()) || controls.is_some_and(|c| c.capture.is_some());
    if feedback.dialog != dialog {
        feedback.dialog = dialog;
    }
    if feedback.key_blocked != blocked {
        feedback.key_blocked = blocked;
    }
}

/// The key pressed with the Escape menu open: the menu goes first (the
/// screenshot shows the match).
fn close_menus(feedback: Res<Feedback>, menu: Option<ResMut<crate::esc_menu::EscMenu>>) {
    if let (Phase::Settling(_), Some(mut m)) = (feedback.phase, menu)
        && m.open
    {
        m.set_open(false);
    }
}

fn clicks(pressed: Query<(&Interaction, &FeedbackAction), Changed<Interaction>>, mut out: MessageWriter<FeedbackAction>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(*a);
        }
    }
}

/// What the dialog last showed: rebuilt when it differs.
#[derive(PartialEq)]
struct Shown {
    category: Category,
    text: String,
    shot: bool,
    include: bool,
    details: bool,
    refused: bool,
    selected: bool,
    units: (u32, u32),
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show(
    feedback: Res<Feedback>,
    outbox: Option<Res<FeedbackOutbox>>,
    root: Query<Entity, With<DialogRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<Shown>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    if !feedback.shown() || !feedback.dialog {
        if shown.take().is_some() {
            for e in &root {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let now = Shown {
        category: feedback.category,
        text: feedback.text.clone(),
        shot: feedback.thumb.is_some(),
        include: feedback.include_screenshot,
        details: feedback.details_open,
        refused: feedback.refused,
        selected: feedback.selected,
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    let ui = Ui::new(theme, kit.as_deref(), *units);
    let dest = outbox.map_or_else(|| "No report service: saved on this PC".to_string(), |o| o.0.destination());
    spawn(&mut commands, &ui, &feedback, &dest);
    *shown = Some(now);
}

/// The dialog under a root of its own over the match.
pub fn spawn(c: &mut Commands, ui: &Ui, f: &Feedback, destination: &str) -> Entity {
    let root = c
        .spawn((
            DialogRoot,
            GlobalZIndex(DIALOG_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ui.color(Tone::Ink, 0.55)),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    let m = ui.modal(c, root, WIDTH, false);
    // The title and the category.
    let top = ui.boxed(c, m, UiStyle { gap: Some(0.8), margin: Some([0.5, 0.0, 1.2, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, top, Tone::Acc, 0.6, 0.9);
    ui.text(c, top, Role::DisplayBold, 1.35, Tone::Fg, "Report");
    ui.boxed(c, top, UiStyle::row().grow(1.0));
    let cats = [Category::Bug, Category::Feedback];
    let sel = cats.iter().position(|k| *k == f.category);
    let seg = ui.segmented(c, top, &["Bug", "Feedback"], sel, State::NONE);
    for (e, k) in seg.into_iter().zip(cats) {
        c.entity(e).insert(FeedbackAction::Category(k));
    }
    ui.kbd(c, top, "Tab", 0.5, Tone::Fg3);
    // The text area: several lines, the caret at the end.
    let area = c
        .spawn((
            UiStyle { h: Some(9.0), pad: Some([0.7, 0.9].into()), ..UiStyle::column() }.bg_a(Tone::Fg, 0.08),
            Node { overflow: Overflow::clip(), ..default() },
            ChildOf(m),
        ))
        .id();
    if f.text.is_empty() {
        let line = ui.boxed(c, area, UiStyle::row());
        caret(c, ui, line);
        ui.text_plain(c, line, Role::Mono, 0.66, Tone::Fg3, " What happened? What did you expect?");
    } else {
        let lines: Vec<&str> = f.text.split('\n').collect();
        for (i, l) in lines.iter().enumerate() {
            let line = ui.boxed(c, area, UiStyle { align: Some(AlignItems::FlexEnd), ..UiStyle::row() });
            let t = c
                .spawn((
                    Text::new(if l.is_empty() { " ".to_string() } else { l.to_string() }),
                    chimera_ui::style::UiText::new(Role::Mono, 0.66, Tone::Fg),
                    chimera_ui::style::PlainText,
                    TextLayout::new(Justify::Left, LineBreak::AnyCharacter),
                    Node { max_width: ui.units.val(WIDTH - 6.0), ..default() },
                    ChildOf(line),
                ))
                .id();
            if f.selected {
                // Ctrl+A: the whole text on the accent tint, as the kit's
                // fields draw a selection.
                c.entity(t).insert((BackgroundColor(ui.color(Tone::Acc, 0.45)), chimera_ui::controls::TextSelection));
            }
            if i + 1 == lines.len() {
                caret(c, ui, line);
            }
        }
    }
    if f.refused {
        ui.underline(c, area, Tone::Threat, 0.2);
    } else {
        ui.underline(c, area, Tone::Acc, 0.2);
    }
    let under = ui.boxed(c, m, UiStyle { margin: Some([0.5, 0.0, 1.1, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::SpaceBetween));
    let hint = if f.refused { "Say what happened first" } else { "Enter: new line" };
    ui.text(c, under, Role::Mono, 0.5, if f.refused { Tone::Threat } else { Tone::Fg3 }, hint);
    ui.text(c, under, Role::Mono, 0.5, Tone::Fg3, &format!("{} / {MAX_TEXT}", f.text.chars().count()));
    // The screenshot and its toggle.
    let shot = ui.boxed(c, m, UiStyle { gap: Some(1.4), margin: Some([0.0, 0.0, 1.0, 0.0].into()), ..UiStyle::row() });
    let (tw, th) = (12.8, 7.2);
    let frame = ui.boxed(c, shot, UiStyle { w: Some(tw), h: Some(th), shrink: Some(0.0), ..UiStyle::row() }.outline(Tone::Fg4, 1.0));
    match &f.thumb {
        Some(img) => {
            c.spawn((
                ImageNode { image: img.clone(), color: Color::WHITE.with_alpha(if f.include_screenshot { 1.0 } else { 0.25 }), ..default() },
                Node { width: percent(100.0), height: percent(100.0), ..default() },
                ChildOf(frame),
            ));
        }
        None => {
            let b = ui.boxed(c, frame, UiStyle { w_pct: Some(100.0), justify: Some(JustifyContent::Center), ..UiStyle::row() });
            ui.text(c, b, Role::Mono, 0.5, Tone::Fg3, "No screenshot");
        }
    }
    let col = ui.boxed(c, shot, UiStyle { gap: Some(0.6), ..UiStyle::column() });
    let trow = ui.boxed(c, col, UiStyle { gap: Some(0.9), ..UiStyle::row() });
    let tg = ui.toggle(c, trow, f.include_screenshot && f.thumb.is_some(), State { disabled: f.thumb.is_none(), ..State::NONE });
    c.entity(tg).insert(FeedbackAction::ToggleScreenshot);
    ui.text(c, trow, Role::DisplayRegular, 0.72, Tone::Fg, "Include screenshot");
    ui.text(c, col, Role::Mono, 0.5, Tone::Fg3, "The frame when you pressed the key");
    // What's sent, folded.
    let head = c
        .spawn((
            UiStyle { gap: Some(0.6), h: Some(1.7), ..UiStyle::row() },
            Node::default(),
            BackgroundColor(Color::NONE),
            chimera_ui::controls::Hover { base: Color::NONE, over: ui.color(Tone::Fg, 0.06) },
            chimera_ui::controls::clickable(),
            FeedbackAction::ToggleDetails,
            ChildOf(m),
        ))
        .id();
    ui.text(c, head, Role::Mono, 0.6, Tone::Fg2, if f.details_open { "-" } else { "+" });
    ui.text(c, head, Role::Mono, 0.56, Tone::Fg2, "What's sent");
    if f.details_open {
        let lines = f.sent_lines();
        let pairs: Vec<(&str, String)> = lines.iter().map(|(k, v)| (*k, v.clone())).collect();
        ui.kv(c, m, &pairs, 7.0);
    }
    let note = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, note, Tone::Acc, 0.42, 0.6);
    ui.text_plain(c, note, Role::Mono, 0.54, Tone::Fg2, NOTICE);
    // Where it goes, and the buttons.
    let foot = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.5, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    ui.text_plain(c, foot, Role::Mono, 0.52, Tone::Fg3, destination);
    ui.boxed(c, foot, UiStyle::row().grow(1.0));
    let cancel = ui.button(c, foot, Kind::Secondary, "Cancel", Some("Esc"), 2.4, State::NONE);
    c.entity(cancel).insert(FeedbackAction::Cancel);
    let send = ui.button(c, foot, Kind::Primary, "Send", Some("Ctrl+Enter"), 2.4, State::focused(true));
    c.entity(send).insert(FeedbackAction::Send);
    root
}

fn caret(c: &mut Commands, ui: &Ui, parent: Entity) {
    c.spawn((
        Node { width: Val::Px(ui.units.hairline() * 2.0), height: ui.units.val(1.0), flex_shrink: 0.0, ..default() },
        BackgroundColor(ui.color(Tone::Acc, 1.0)),
        chimera_ui::controls::TextCaret,
        Pickable::IGNORE,
        ChildOf(parent),
    ));
}

/// The notice after Send, at the top for a few seconds.
#[allow(clippy::type_complexity)]
fn show_notice(
    feedback: Res<Feedback>,
    time: Res<Time<Real>>,
    root: Query<Entity, With<NoticeRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<(String, (u32, u32))>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let live = feedback
        .notice
        .as_ref()
        .filter(|n| feedback.dialog && time.elapsed_secs_f64() < n.until)
        .map(|n| (n.text.clone(), ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32)));
    if *shown == live {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((text, _)) = &live {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let r = commands
            .spawn((
                NoticeRoot,
                GlobalZIndex(DIALOG_Z),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    // Under the race chip at the top.
                    top: ui.units.val(11.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        let b = ui.boxed(&mut commands, r, UiStyle { gap: Some(0.7), pad: Some([0.8, 1.4].into()), ..UiStyle::row() }.bg(Tone::Ink2).outline(Tone::Fg4, 1.0));
        ui.focus_bar(&mut commands, b, 0.22);
        ui.mark(&mut commands, b, Tone::Acc, 0.5, 0.7);
        ui.text(&mut commands, b, Role::Mono, 0.62, Tone::Fg, text);
    }
    *shown = live;
}

/// `ui_scene report` (screenshots): the dialog open with made-up words
/// (over the frame a `feedback` took, else a made-up one); `report
/// details` unfolds What's sent; `report sent` and `report queued` show
/// the notice after Send; `off` closes it.
fn scene_report(w: &mut World, args: &[String]) -> Result<(), String> {
    use chimera_client::client::feedback::{Notice, fake_frame};
    let arg = args.first().map(String::as_str);
    if arg == Some("off") {
        let mut f = w.resource_mut::<Feedback>();
        f.phase = Phase::Closed;
        f.notice = None;
        return Ok(());
    }
    if matches!(arg, Some("sent" | "queued")) {
        let now = w.resource::<Time<Real>>().elapsed_secs_f64();
        let sent = arg == Some("sent");
        let text = if sent { "Report sent" } else { "Saved, sends when the service answers (2 waiting)" };
        let mut f = w.resource_mut::<Feedback>();
        f.phase = Phase::Closed;
        f.notice = Some(Notice { text: text.into(), until: now + 600.0, sent });
        return Ok(());
    }
    if w.resource::<Feedback>().thumb.is_none() {
        let snap = chimera_client::client::capture::report_text(w, "");
        let thumb = w.resource_mut::<Assets<Image>>().add(fake_frame());
        let mut f = w.resource_mut::<Feedback>();
        f.snapshot = snap;
        f.shot = Some(fake_frame());
        f.thumb = Some(thumb);
        f.include_screenshot = true;
    }
    let mut f = w.resource_mut::<Feedback>();
    f.dialog = true;
    f.phase = Phase::Open;
    f.text = "Fell through the floor on the ramp by B site after the swap to the AK.\nHappened twice; the second time I was stuck under the map.".into();
    f.details_open = arg == Some("details");
    Ok(())
}
