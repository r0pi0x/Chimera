use std::collections::HashMap;

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::{ComputedNode, UiGlobalTransform},
    window::PrimaryWindow,
};

use crate::client::fonts::UiFonts;
use super::*;
use bevy::ui::Interaction;
use crate::console::ConsoleAppExt;
use crate::map::hud::GameUi;

// ---------------------------------------------------------------------------
// The look and the drawing helpers.

/// Colours, sizes and fonts: the GameUI scheme's, else built in.
pub struct Look<'a> {
    pub ui: Option<&'a GameUi>,
    pub fonts: &'a UiFonts,
    /// Pixels per scheme pixel (GameUI is drawn in screen pixels; larger
    /// windows scale it up).
    pub s: f32,
    pub height: f32,
    pub accent: Color,
    /// Our faces at the scheme's sizes (`UiFonts::ours`): a menu of our
    /// own drawn in a game's GameUI look (the party apps'); `false` for the
    /// game's own menus, in its scheme's faces.
    pub own_faces: bool,
}

impl<'a> Look<'a> {
    pub fn color(&self, name: &str, fallback: [u8; 4]) -> Color {
        let [r, g, b, a] = self.ui.and_then(|u| u.color(name)).unwrap_or(fallback);
        Color::srgba_u8(r, g, b, a)
    }

    pub fn number(&self, name: &str, fallback: f32) -> f32 {
        self.ui.and_then(|u| u.numbers.get(name).copied()).unwrap_or(fallback)
    }

    /// Built in colours stand in for the scheme's: a darker panel, our
    /// accent for selections.
    pub fn has_scheme(&self) -> bool {
        self.ui.is_some_and(|u| !u.colors.is_empty())
    }

    /// A GameUI scheme font (`Default`, `UiBold`, `MenuLarge`) at this
    /// window's size: `fallback` scheme pixels tall and bold when the
    /// scheme lacks it.
    pub fn font(&self, name: &str, fallback: (f32, bool)) -> TextFont {
        let f = self.fonts.source(name, self.height, self.s, fallback);
        if self.own_faces {
            let bold = self.fonts.sizes(crate::client::fonts::Scheme::Source, name).and_then(|s| s.first()).map_or(fallback.1, |s| s.bold());
            self.fonts.ours(f, bold)
        } else {
            f
        }
    }

    /// The dialogs' text font.
    pub fn default_font(&self) -> TextFont {
        self.font("Default", (16.0, false))
    }

    pub fn frame_bg(&self) -> Color {
        if self.has_scheme() {
            self.color("Frame.BgColor", [160, 160, 160, 128])
        } else {
            Color::srgba_u8(28, 28, 28, 230)
        }
    }

    pub fn bright(&self) -> Color {
        self.color("Border.Bright", [200, 200, 200, 196])
    }

    pub fn dark(&self) -> Color {
        self.color("Border.Dark", [40, 40, 40, 196])
    }

    pub fn text(&self) -> Color {
        self.color("Label.TextColor", [221, 221, 221, 255])
    }

    pub fn dull(&self) -> Color {
        self.color("Label.TextDullColor", [190, 190, 190, 255])
    }

    pub fn disabled(&self) -> Color {
        self.color("Label.DisabledFgColor1", [117, 117, 117, 255])
    }

    pub fn white(&self) -> Color {
        self.color("Label.TextBrightColor", [255, 255, 255, 255])
    }

    pub fn selected_bg(&self) -> Color {
        if self.has_scheme() {
            self.color("SectionedListPanel.SelectedBgColor", [255, 155, 0, 255])
        } else {
            self.accent
        }
    }

    pub fn selected_text(&self) -> Color {
        self.color("SectionedListPanel.SelectedTextColor", [0, 0, 0, 255])
    }

    pub fn sunken_bg(&self) -> Color {
        self.color("TextEntry.BgColor", [0, 0, 0, 128])
    }

    pub fn px(&self, v: f32) -> Val {
        px((v * self.s).round())
    }
}

/// An absolutely placed box, in scheme pixels, inside `parent`.
pub fn place(look: &Look, x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: look.px(x),
        top: look.px(y),
        width: look.px(w),
        height: look.px(h),
        ..default()
    }
}

/// Raised (lit top-left) or sunken (lit bottom-right) VGUI borders.
pub fn bevel(look: &Look, raised: bool) -> BorderColor {
    let (a, b) = if raised {
        (look.bright(), look.dark())
    } else {
        (look.dark(), look.bright())
    };
    BorderColor {
        top: a,
        left: a,
        bottom: b,
        right: b,
    }
}

/// Text in a box: one line, vertically centred, `align` -1 left, 0
/// centre, 1 right.
#[allow(clippy::too_many_arguments)]
pub fn label(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    font: TextFont,
    color: Color,
    align: i32,
) -> Entity {
    let node = Node {
        display: Display::Flex,
        align_items: AlignItems::Center,
        justify_content: match align {
            -1 => JustifyContent::FlexStart,
            0 => JustifyContent::Center,
            _ => JustifyContent::FlexEnd,
        },
        overflow: Overflow::clip(),
        ..place(look, x, y, w, h)
    };
    let e = commands.spawn((node, ChildOf(parent))).id();
    commands.spawn((
        Text::new(text),
        font,
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ChildOf(e),
    ));
    e
}

/// Disabled text as VGUI draws it: the second disabled colour a pixel
/// down and right, the first over it.
#[allow(clippy::too_many_arguments)]
pub fn engraved(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    font: TextFont,
    align: i32,
) {
    let under = look.color("Label.DisabledFgColor2", [30, 30, 30, 255]);
    label(
        commands,
        parent,
        look,
        (x + 1.0, y + 1.0, w, h),
        text,
        font.clone(),
        under,
        align,
    );
    label(commands, parent, look, (x, y, w, h), text, font, look.disabled(), align);
}

/// How a frame is drawn: its id, whether it's modal (stacked over the
/// rest, solid), sizeable down to a minimum (scheme pixels), and has a
/// close box.
#[derive(Clone, Copy, Debug)]
pub struct FrameSpec {
    pub id: WindowId,
    pub modal: bool,
    pub sizeable: Option<Vec2>,
    pub close: bool,
}

impl FrameSpec {
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            modal: false,
            sizeable: None,
            close: true,
        }
    }

    pub fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    pub fn sizeable(mut self, min: Vec2) -> Self {
        self.sizeable = Some(min);
        self
    }

    pub fn no_close(mut self) -> Self {
        self.close = false;
        self
    }
}

/// The close box of a frame drawn by `frame` (its owner gives it a hit).
pub struct FrameParts {
    pub frame: Entity,
    pub close: Option<Entity>,
}

/// A GameUI frame (`Frame`): its background, raised borders, title, the
/// close box at the top right and, sizeable, the grip at the bottom right;
/// placed (centred the first time, then where it was dragged) and stacked
/// by `place_frames`. Children go in scheme pixels from its corner.
pub fn frame(
    commands: &mut Commands,
    root: Entity,
    look: &Look,
    spec: FrameSpec,
    (w, h): (f32, f32),
    title: &str,
) -> FrameParts {
    let bg = look.frame_bg();
    let bg = if spec.modal {
        // Solid: the dialog under a modal one would show through the
        // scheme's see-through frame colour.
        let c = bg.to_srgba();
        Color::srgb(c.red * 0.7, c.green * 0.7, c.blue * 0.7)
    } else {
        bg
    };
    let e = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, -10_000.0 / look.s, 0.0, w, h)
            },
            bevel(look, true),
            // What the frame hides (`FrameBacking`, set by `place_frames`),
            // its see-through colour over it.
            BackgroundColor(Color::NONE),
            VguiFrame {
                id: spec.id,
                size: Vec2::new(w, h),
                scale: look.s,
                sizeable: spec.sizeable,
                modal: spec.modal,
            },
            // Clicks on the frame stop at it (not the dialog under it).
            bevy::ui::FocusPolicy::Block,
            Interaction::default(),
            GlobalZIndex(FRAME_Z),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        BackgroundColor(bg),
        ChildOf(e),
    ));
    let inset = look.number("Frame.TitleTextInsetX", 16.0);
    label(
        commands,
        e,
        look,
        (inset, 4.0, w - inset - 32.0, 22.0),
        title,
        look.font("UiBold", (12.0, true)),
        look.color("FrameTitleBar.TextColor", [255, 255, 255, 255]),
        -1,
    );
    let close = spec.close.then(|| {
        let b = commands
            .spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    ..place(look, w - 24.0, 6.0, 18.0, 16.0)
                },
                bevel(look, true),
                BackgroundColor(look.color("Button.BgColor", [0, 0, 0, 0])),
                clickable(),
                ChildOf(e),
            ))
            .id();
        // Marlett's close glyph, else an x.
        label(
            commands,
            b,
            look,
            (0.0, 0.0, 16.0, 14.0),
            "\u{00D7}",
            look.font("Default", (14.0, false)),
            look.color("FrameTitleBar.TextColor", [255, 255, 255, 255]),
            0,
        );
        b
    });
    if spec.sizeable.is_some() {
        // The grip: three short diagonals at the corner.
        let c = look.color("FrameGrip.Color1", [200, 200, 200, 196]);
        let d = look.color("FrameGrip.Color2", [40, 40, 40, 196]);
        for k in 0..3 {
            let o = 4.0 + 4.0 * k as f32;
            for (dx, color) in [(0.0, c), (1.0, d)] {
                commands.spawn((
                    place(look, w - o + dx - 2.0, h - 4.0 - dx, 2.0, 2.0),
                    BackgroundColor(color),
                    ChildOf(e),
                ));
                commands.spawn((
                    place(look, w - 4.0 - dx, h - o + dx - 2.0, 2.0, 2.0),
                    BackgroundColor(color),
                    ChildOf(e),
                ));
            }
        }
    }
    FrameParts { frame: e, close }
}

/// A button's states' colours, kept on it for `style_buttons`.
#[derive(Component, Clone, Copy, Debug)]
pub struct ButtonStyle {
    pub enabled: bool,
    /// Depressed for good (a toggle's on state).
    pub latched: bool,
    pub bg: [Color; 3],
    pub fg: [Color; 3],
    pub raised: BorderColor,
    pub sunken: BorderColor,
    pub text: Entity,
}

/// How a button looks: its text, whether it works, is the dialog's
/// default (Enter presses it) or has the keyboard focus, or stays pressed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonState {
    pub enabled: bool,
    pub default: bool,
    pub focused: bool,
    pub latched: bool,
}

impl ButtonState {
    pub fn enabled(enabled: bool) -> Self {
        Self { enabled, ..default() }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn default_button(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    pub fn latched(mut self, latched: bool) -> Self {
        self.latched = latched;
        self
    }
}

/// A VGUI `Button`: raised, its text; armed (hovered) and depressed in the
/// scheme's colours (`style_buttons`), sunken while held; the default
/// button and the focused one ringed; disabled text engraved and no hit.
/// `hit` goes on it when enabled (the owner's click target).
#[allow(clippy::too_many_arguments)]
pub fn button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    state: ButtonState,
    align: i32,
    hit: impl Bundle,
) -> Entity {
    let bg = [
        look.color("Button.BgColor", [0, 0, 0, 0]),
        look.color("Button.ArmedBgColor", [255, 255, 255, 24]),
        look.color("Button.DepressedBgColor", [0, 0, 0, 64]),
    ];
    let fg = [
        look.color("Button.TextColor", [255, 255, 255, 255]),
        look.color("Button.ArmedTextColor", [255, 255, 255, 255]),
        look.color("Button.DepressedTextColor", [255, 255, 255, 255]),
    ];
    let raised = bevel(look, true);
    let sunken = bevel(look, false);
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, x, y, w, h)
        },
        if state.latched { sunken } else { raised },
        BackgroundColor(if state.latched { bg[2] } else { bg[0] }),
        ChildOf(parent),
    ));
    if state.default || state.focused {
        e.insert(Outline::new(
            px(1.0),
            px(0.0),
            look.color("Button.FocusBorderColor", [0, 0, 0, 255]),
        ));
    }
    let e = e.id();
    let font = look.default_font();
    let text_e = if state.enabled {
        label(
            commands,
            e,
            look,
            (6.0, 0.0, w - 12.0, h - 2.0),
            text,
            font,
            fg[0],
            align,
        )
    } else {
        engraved(commands, e, look, (6.0, 0.0, w - 12.0, h - 2.0), text, font, align);
        e
    };
    if state.enabled {
        commands.entity(e).insert((
            clickable(),
            ButtonStyle {
                enabled: true,
                latched: state.latched,
                bg,
                fg,
                raised,
                sunken,
                text: text_e,
            },
            hit,
        ));
    }
    e
}

/// Buttons as the pointer arms (hovers) and presses them: the scheme's
/// armed and depressed colours, sunken while held.
pub(super) fn style_buttons(
    mut buttons: Query<(&Interaction, &ButtonStyle, &mut BackgroundColor, &mut BorderColor), Changed<Interaction>>,
    children: Query<&Children>,
    mut texts: Query<&mut TextColor>,
) {
    for (interaction, style, mut bg, mut border) in &mut buttons {
        let state = match interaction {
            _ if style.latched => 2,
            Interaction::None => 0,
            Interaction::Hovered => 1,
            Interaction::Pressed => 2,
        };
        bg.0 = style.bg[state];
        *border = if state == 2 { style.sunken } else { style.raised };
        if let Ok(kids) = children.get(style.text) {
            for k in kids.iter() {
                if let Ok(mut t) = texts.get_mut(k) {
                    t.0 = style.fg[state];
                }
            }
        }
    }
}

/// A `CheckButton`: a sunken box (ticked when on) and its text, the whole
/// of it clicked; greyed when disabled.
#[allow(clippy::too_many_arguments)]
pub fn check_button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    text: &str,
    on: bool,
    enabled: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    check_like(commands, parent, look, rect, text, (on, enabled, focused, false), hit)
}

/// A `RadioButton`: a check button with a round box, one of a group.
#[allow(clippy::too_many_arguments)]
pub fn radio_button(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    text: &str,
    on: bool,
    enabled: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    check_like(commands, parent, look, rect, text, (on, enabled, focused, true), hit)
}

pub(super) fn check_like(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    (on, enabled, focused, round): (bool, bool, bool, bool),
    hit: impl Bundle,
) -> Entity {
    let radius = if round { BorderRadius::MAX } else { BorderRadius::ZERO };
    let mut e = commands.spawn((place(look, x, y, w, h), ChildOf(parent)));
    if enabled {
        e.insert((clickable(), hit));
    }
    if focused {
        e.insert(Outline::new(
            px(1.0),
            px(0.0),
            look.color("Border.Dark", [40, 40, 40, 196]),
        ));
    }
    let e = e.id();
    let b = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                border_radius: radius,
                ..place(look, 2.0, ((h - 14.0) / 2.0).round(), 14.0, 14.0)
            },
            BorderColor {
                top: look.color("CheckButton.Border1", [40, 40, 40, 196]),
                left: look.color("CheckButton.Border1", [40, 40, 40, 196]),
                bottom: look.color("CheckButton.Border2", [200, 200, 200, 196]),
                right: look.color("CheckButton.Border2", [200, 200, 200, 196]),
            },
            BackgroundColor(look.color("CheckButton.BgColor", [0, 0, 0, 128])),
            ChildOf(e),
        ))
        .id();
    if on {
        let check = if enabled {
            look.color("CheckButton.Check", [255, 255, 255, 255])
        } else {
            look.color("CheckButton.DisabledFgColor", [117, 117, 117, 255])
        };
        // Marlett's tick (a dot for a radio button); a square stands in.
        commands.spawn((
            Node {
                border_radius: radius,
                ..place(look, 3.0, 3.0, 6.0, 6.0)
            },
            BackgroundColor(check),
            ChildOf(b),
        ));
    }
    let font = look.default_font();
    // The words run on past a narrow box (VGUI sizes the label to them).
    let text_w = (look.fonts.char_offsets(&font, text).last().copied().unwrap_or(0.0) / look.s + 4.0).max(w - 22.0);
    if text.is_empty() {
    } else if enabled {
        let color = look.color("CheckButton.TextColor", [255, 255, 255, 255]);
        label(commands, e, look, (22.0, 0.0, text_w, h), text, font, color, -1);
    } else {
        engraved(commands, e, look, (22.0, 0.0, text_w, h), text, font, -1);
    }
    e
}

/// A `ComboBox` closed: a sunken box with the value (left), the arrow
/// button at its right (`ComboBoxButton`, raised; sunken while its list
/// is open). `hit` goes on the whole box (a click opens the list).
#[allow(clippy::too_many_arguments)]
pub fn combo_box(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    rect: (f32, f32, f32, f32),
    value: &str,
    enabled: bool,
    open: bool,
    focused: bool,
    hit: impl Bundle,
) -> Entity {
    combo_box_aligned(commands, parent, look, rect, value, enabled, open, focused, -1, hit)
}

/// `combo_box` with its value aligned as its `textAlignment` says: -1
/// west (left), 0 centre, 1 east.
#[allow(clippy::too_many_arguments)]
pub fn combo_box_aligned(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    value: &str,
    enabled: bool,
    open: bool,
    focused: bool,
    align: i32,
    hit: impl Bundle,
) -> Entity {
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, x, y, w, h)
        },
        bevel(look, false),
        BackgroundColor(look.sunken_bg()),
        ChildOf(parent),
    ));
    if enabled {
        e.insert((clickable(), hit));
    }
    let e = e.id();
    let font = look.default_font();
    let text_w = w - 22.0;
    if !enabled {
        engraved(commands, e, look, (4.0, 0.0, text_w - 4.0, h - 2.0), value, font, align);
    } else if focused && !open {
        // Focused: the value shown selected, as VGUI's text entry.
        let at = label(
            commands,
            e,
            look,
            (2.0, 2.0, text_w - 2.0, h - 6.0),
            "",
            font.clone(),
            Color::NONE,
            -1,
        );
        commands.entity(at).insert(BackgroundColor(
            look.color("TextEntry.SelectedBgColor", [255, 155, 0, 255]),
        ));
        label(
            commands,
            e,
            look,
            (4.0, 0.0, text_w - 4.0, h - 2.0),
            value,
            font,
            look.color("TextEntry.SelectedTextColor", [0, 0, 0, 255]),
            align,
        );
    } else {
        let color = look.color("TextEntry.TextColor", [221, 221, 221, 255]);
        label(
            commands,
            e,
            look,
            (4.0, 0.0, text_w - 4.0, h - 2.0),
            value,
            font,
            color,
            align,
        );
    }
    let a = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, w - 20.0, 1.0, 18.0, h - 4.0)
            },
            bevel(look, !open),
            BackgroundColor(look.color("ComboBoxButton.BgColor", [0, 0, 0, 0])),
            ChildOf(e),
        ))
        .id();
    let arrow = if enabled {
        look.color("ComboBoxButton.ArrowColor", [190, 190, 190, 255])
    } else {
        look.color("ComboBoxButton.DisabledBgColor", [117, 117, 117, 255])
    };
    label(
        commands,
        a,
        look,
        (0.0, 0.0, 16.0, h - 6.0),
        "\u{25BC}",
        look.font("Marlett", (9.0, false)),
        arrow,
        0,
    );
    e
}

/// A combo box's open list (VGUI's `Menu`), under its box at (x, y + h)
/// in `parent`: the entries shown, the highlighted one lit, a scroll bar
/// past `COMBO_ROWS`; each entry gets `hit(i)`; behind it, the whole
/// screen catches a click outside (`outside`).
#[allow(clippy::too_many_arguments)]
pub fn combo_popup<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    root: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    entries: &[String],
    list: &ComboList,
    hit: impl Fn(usize) -> B,
    outside: impl Bundle,
) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        // Blocking (`clickable`): a click passing through reached the
        // box under it too, which opened the list again as the catcher
        // closed it.
        clickable(),
        outside,
        GlobalZIndex(POPUP_Z - 1),
        ChildOf(root),
    ));
    let row_h = 20.0;
    let rows = list.rows();
    let bar = if entries.len() > rows {
        look.number("ScrollBar.Wide", 17.0)
    } else {
        0.0
    };
    // A solid backing (the scheme's menu colour is see-through), the
    // menu's colour over it.
    let backing = {
        let c = look.frame_bg().to_srgba();
        Color::srgb(c.red * 0.45, c.green * 0.45, c.blue * 0.45)
    };
    let solid = commands
        .spawn((
            place(look, x, y + h, w, rows as f32 * row_h + 2.0),
            BackgroundColor(backing),
            bevy::ui::FocusPolicy::Block,
            GlobalZIndex(POPUP_Z),
            ChildOf(parent),
        ))
        .id();
    let menu = commands
        .spawn((
            Node {
                border: UiRect::all(px(1.0)),
                ..place(look, 0.0, 0.0, w, rows as f32 * row_h + 2.0)
            },
            BorderColor::all(look.color("Menu.BorderColor", [0, 0, 0, 255])),
            BackgroundColor(look.color("Menu.BgColor", [40, 40, 40, 255])),
            ChildOf(solid),
        ))
        .id();
    let font = look.default_font();
    for (k, i) in (list.first..(list.first + rows).min(entries.len())).enumerate() {
        let lit = i == list.highlight;
        let e = commands
            .spawn((
                place(look, 0.0, k as f32 * row_h, w - 2.0 - bar, row_h),
                BackgroundColor(if lit {
                    look.color("Menu.ArmedBgColor", [255, 155, 0, 255])
                } else {
                    Color::NONE
                }),
                clickable(),
                hit(i),
                ChildOf(menu),
            ))
            .id();
        let color = if lit {
            look.color("Menu.ArmedTextColor", [0, 0, 0, 255])
        } else {
            look.color("Menu.TextColor", [221, 221, 221, 255])
        };
        label(
            commands,
            e,
            look,
            (4.0, 0.0, w - 10.0 - bar, row_h),
            &entries[i],
            font.clone(),
            color,
            -1,
        );
    }
    if bar > 0.0 {
        let track = commands
            .spawn((
                place(look, w - 2.0 - bar, 0.0, bar, rows as f32 * row_h),
                BackgroundColor(look.sunken_bg()),
                ChildOf(menu),
            ))
            .id();
        let room = rows as f32 * row_h;
        if let Some((top, thumb)) = scroll_thumb(entries.len(), rows, list.first, room) {
            // Dragged, it scrolls the list (`COMBO_SCROLL`).
            commands.spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    ..place(look, 1.0, top, bar - 2.0, thumb)
                },
                bevel(look, true),
                BackgroundColor(look.color("ScrollBarSlider.BgColor", [255, 255, 255, 64])),
                ScrollThumb {
                    owner: COMBO_SCROLL,
                    id: list.owner,
                    len: entries.len(),
                    shown: rows,
                    room,
                    top,
                    thumb,
                },
                clickable(),
                ChildOf(track),
            ));
        }
    }
}

/// A `Slider`: its sunken track, the nob at `fraction`, `ticks` marks
/// under it (VGUI's `numTicks`; none at 0); pressing or dragging along it
/// sets the value (`SliderDrag`).
#[allow(clippy::too_many_arguments)]
pub fn slider(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    fraction: f32,
    ticks: usize,
    enabled: bool,
    track: SliderTrack,
) -> Entity {
    const NOB: f32 = 8.0;
    let mut e = commands.spawn((place(look, x, y, w, h), ChildOf(parent)));
    if enabled {
        e.insert((
            SliderTrack { inset: NOB, ..track },
            clickable(),
            bevy::ui::RelativeCursorPosition::default(),
        ));
    }
    let e = e.id();
    let mid = (h / 2.0 - 4.0).round();
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, NOB / 2.0, mid, w - NOB, 4.0)
        },
        bevel(look, false),
        BackgroundColor(look.color("Slider.TrackColor", [31, 31, 31, 255])),
        ChildOf(e),
    ));
    if ticks > 1 {
        let tick = look.color("Slider.TextColor", [127, 140, 127, 255]);
        for k in 0..ticks {
            let tx = NOB / 2.0 + (w - NOB) * k as f32 / (ticks - 1) as f32;
            commands.spawn((
                place(look, tx.round() - 0.5, mid + 9.0, 1.0, 4.0),
                BackgroundColor(tick),
                ChildOf(e),
            ));
        }
    }
    let nob_x = (w - NOB) * fraction.clamp(0.0, 1.0);
    commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            ..place(look, nob_x, mid - 6.0, NOB, 16.0)
        },
        bevel(look, true),
        BackgroundColor(if enabled {
            look.color("Slider.NobColor", [108, 108, 108, 255])
        } else {
            look.color("Slider.DisabledTextColor1", [117, 117, 117, 255])
        }),
        ChildOf(e),
    ));
    e
}

/// A `TextEntry`: sunken, its text (dots for a password), scrolled so the
/// caret shows; focused: the selection highlighted and the caret drawn.
/// `hit` goes on it (a click focuses it and places the caret).
#[allow(clippy::too_many_arguments)]
pub fn text_entry(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y, w, h): (f32, f32, f32, f32),
    text: &str,
    caret: Option<Caret>,
    hidden: bool,
    enabled: bool,
    hit: impl Bundle,
) -> Entity {
    let mut e = commands.spawn((
        Node {
            border: UiRect::all(px(1.0)),
            overflow: Overflow::clip(),
            ..place(look, x, y, w, h)
        },
        bevel(look, false),
        BackgroundColor(look.sunken_bg()),
        ChildOf(parent),
    ));
    if enabled {
        e.insert((
            clickable(),
            bevy::ui::RelativeCursorPosition::default(),
            hit,
        ));
    }
    let e = e.id();
    let shown: String = if hidden {
        "*".repeat(text.chars().count())
    } else {
        text.to_string()
    };
    let font = look.default_font();
    let offsets = look.fonts.char_offsets(&font, &shown);
    // Scheme pixels of the text before char i.
    let at = |i: usize| offsets.get(i).copied().unwrap_or(0.0) / look.s;
    let inner = w - 8.0;
    let scroll = match caret {
        Some(c) => (at(c.at) - inner + 2.0).max(0.0),
        None => 0.0,
    };
    let color = if enabled {
        look.color("TextEntry.TextColor", [221, 221, 221, 255])
    } else {
        look.disabled()
    };
    if let Some(c) = caret.filter(|c| c.at != c.anchor) {
        let r = c.range();
        let (sx, ex) = (at(r.start) - scroll, at(r.end) - scroll);
        commands.spawn((
            place(look, 4.0 + sx, 3.0, (ex - sx).max(1.0), h - 8.0),
            BackgroundColor(look.color("TextEntry.SelectedBgColor", [255, 155, 0, 255])),
            ChildOf(e),
        ));
    }
    label(
        commands,
        e,
        look,
        (4.0 - scroll, 0.0, inner + scroll, h - 2.0),
        &shown,
        font,
        color,
        -1,
    );
    if let Some(c) = caret {
        commands.spawn((
            place(look, 4.0 + at(c.at) - scroll, 4.0, 1.0, h - 10.0),
            BackgroundColor(look.color("TextEntry.CursorColor", [255, 255, 255, 255])),
            ChildOf(e),
        ));
    }
    e
}

/// The char a click at `x` (0 to 1 across a text entry `w` scheme pixels
/// wide) lands at, the entry's text scrolled as `text_entry` draws it.
pub fn caret_from_click(
    look_fonts: &UiFonts,
    font: &TextFont,
    scale: f32,
    shown: &str,
    w: f32,
    fraction: f32,
    caret: Option<Caret>,
) -> usize {
    let offsets = look_fonts.char_offsets(font, shown);
    let at = |i: usize| offsets.get(i).copied().unwrap_or(0.0) / scale;
    let inner = w - 8.0;
    let scroll = caret.map_or(0.0, |c| (at(c.at) - inner + 2.0).max(0.0));
    let x = (fraction * w - 4.0 + scroll) * scale;
    char_at(&offsets, x)
}

/// A property sheet's tabs (`PropertySheet`): raised, the open one joined
/// to the page under it; each gets `hit(i)` (`enabled` false: greyed and
/// no hit). `widths`: each tab's width, else fitted to its text.
#[allow(clippy::too_many_arguments)]
pub fn tabs<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    look: &Look,
    (x, y): (f32, f32),
    names: &[(String, bool)],
    open: usize,
    width: Option<f32>,
    fit: Option<f32>,
    hit: impl Fn(usize) -> B,
) {
    let font = look.default_font();
    let text: Vec<f32> = names
        .iter()
        .map(|(name, _)| look.fonts.char_offsets(&font, name).last().copied().unwrap_or(0.0) / look.s)
        .collect();
    let widths = tab_widths(&text, width, fit);
    let mut tx = x;
    for (i, (name, enabled)) in names.iter().enumerate() {
        let tab_w = widths[i];
        let is_open = i == open;
        let (ty, th) = if is_open { (y, 27.0) } else { (y + 3.0, 24.0) };
        let mut e = commands.spawn((
            Node {
                border: UiRect {
                    left: px(1.0),
                    right: px(1.0),
                    top: px(1.0),
                    bottom: px(0.0),
                },
                ..place(look, tx, ty, tab_w - 2.0, th)
            },
            bevel(look, true),
            BackgroundColor(if is_open { look.frame_bg() } else { Color::NONE }),
            ZIndex(if is_open { 2 } else { 0 }),
            ChildOf(parent),
        ));
        if *enabled {
            e.insert((clickable(), hit(i)));
        }
        let e = e.id();
        let color = if is_open {
            look.color("PropertySheet.SelectedTextColor", [255, 255, 255, 255])
        } else {
            look.color("PropertySheet.TextColor", [221, 221, 221, 255])
        };
        if *enabled {
            label(
                commands,
                e,
                look,
                (0.0, 0.0, tab_w - 4.0, th - 1.0),
                name,
                font.clone(),
                color,
                0,
            );
        } else {
            engraved(
                commands,
                e,
                look,
                (0.0, 0.0, tab_w - 4.0, th - 1.0),
                name,
                font.clone(),
                0,
            );
        }
        tx += tab_w;
    }
}

/// Each tab's width in scheme pixels: `width` each, else its words'
/// width (`text`) plus 12 on each side, at least 56; tabs that would
/// reach past `fit` (the sheet's width) shrink in proportion so the last
/// one ends inside it.
pub fn tab_widths(text: &[f32], width: Option<f32>, fit: Option<f32>) -> Vec<f32> {
    let natural: Vec<f32> = text
        .iter()
        .map(|t| width.unwrap_or_else(|| (t + 24.0).max(56.0).round()))
        .collect();
    let total: f32 = natural.iter().sum();
    match fit {
        Some(fit) if total > fit && total > 0.0 => natural.iter().map(|w| (w * fit / total).floor()).collect(),
        _ => natural,
    }
}

/// The wheel's notches this frame (up positive).
pub fn wheel_notches(scroll: &bevy::input::mouse::AccumulatedMouseScroll) -> i32 {
    use bevy::input::mouse::MouseScrollUnit;
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y.round(),
        MouseScrollUnit::Pixel => (scroll.delta.y / 40.0).round(),
    };
    notches as i32
}

/// Bevy's UI as the game runs it (layout, stacking, focus: what the
/// pointer presses) in a 400x300 window, with `block_clicks`, the
/// pointer driven by `PointerScript`; nothing drawn. For tests of what a
/// press reaches through the menus' stacking (here and the games' menus).
pub fn pointer_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        },
        TransformPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ))
    // What the UI's picking backend reads (the focus system is what
    // the menus use).
    .add_plugins((bevy::picking::PickingPlugin, bevy::picking::InteractionPlugin))
    .init_asset::<bevy::image::TextureAtlasLayout>()
    // Hidden nodes take no clicks: the focus system reads visibility.
    .add_plugins(bevy::camera::visibility::VisibilityPlugin)
    .init_asset::<Mesh>()
    .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
    .init_resource::<PointerScript>()
    .add_observer(block_clicks)
    .add_systems(PreUpdate, run_pointer_script.before(bevy::input::InputSystems));
    let mut window = Window::default();
    window.resolution.set(400.0, 300.0);
    app.world_mut().spawn((window, PrimaryWindow));
    let size = UVec2::new(400, 300);
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: bevy::camera::ComputedCameraValues {
                target_info: Some(bevy::camera::RenderTargetInfo {
                    physical_size: size,
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(bevy::camera::Viewport {
                physical_size: size,
                ..default()
            }),
            ..default()
        },
    ));
    app
}

