//! Small builders on Bevy UI from the theme (docs/plans/active/ugg-ui.md,
//! 2.1; the player marks as Look Book V settled them):
//!
//! - `tag`: solid fills in Mono, YOU white, HOST cobalt (`TagKind`), and
//!   `spawn_you_host`: a player who is both gets one chip split on the
//!   12° slant (white YOU, then cobalt HOST).
//! - `bot_slot`: the slot reserved left of every player's name, holding
//!   the antenna robot icon for a bot (no BOT tag), empty otherwise.
//! - `row_you`: your row (7 % light tint and a 1 px outline at 60 %);
//!   `hover_tick` + `HoverTicks`: a hoverable row's hover is a slanted tick
//!   at its right edge (shown while the row is `Hovered`), never an
//!   outline, so it never reads as YOU.
//! - `kbd`: a key cap.
//! - `spawn_ping_meter` / `PingLevel`: four slanted bars, lit by ping
//!   (the theme's `[network]` thresholds: quiet in `Good`, fine, amber
//!   `Warn`, else `Threat`), bots none; the `loss` icon beside it.
//!
//! Builders return bundles the caller spawns (`spawn_pair` for a box and
//! its text); those needing a material spawn themselves.

use bevy::{picking::hover::Hovered, prelude::*};

use crate::{
    fonts::{self, KitFonts, Role},
    icons::IconImages,
    materials::SlantMaterial,
    theme::{Theme, Tone},
    units::UiUnits,
};

/// A tag's fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    /// You: white fill, ink text.
    You,
    /// The host: cobalt fill.
    Host,
    /// Accent fill (a rung, the leader); the same as HOST's.
    Accent,
}

/// Tags' text size on rows, cq (the board's; the lobby's are .62).
pub const TAG_CQ: f32 = 0.42;

fn tag_colors(theme: &Theme, kind: TagKind) -> (Color, Color) {
    match kind {
        TagKind::You => (theme.tone(Tone::Fg), theme.tone(Tone::Ink)),
        TagKind::Host | TagKind::Accent => (theme.tone(Tone::Acc), theme.tone(Tone::OnAcc)),
    }
}

fn tag_label(theme: &Theme, kit: Option<&KitFonts>, units: &UiUnits, text: &str, size_cq: f32, color: Color) -> impl Bundle + use<> {
    let s = fonts::text(theme, kit, Role::Mono, units.text_px(size_cq));
    // Tags track wider than Mono's text (.1 em).
    let spacing = bevy::text::LetterSpacing::Px(0.1 * units.text_px(size_cq));
    (Text::new(text.to_uppercase()), s.font.clone(), spacing, s.hinting, TextColor(color))
}

/// A tag's box and its text: padding .26/.4/.2 em-ish (the look book's
/// .26 .4 .2 cq at .62 cq, scaled to the size).
pub fn tag(
    theme: &Theme,
    kit: Option<&KitFonts>,
    units: &UiUnits,
    kind: TagKind,
    text: &str,
    size_cq: f32,
) -> (impl Bundle + use<>, impl Bundle + use<>) {
    let k = size_cq / 0.62;
    let (bg, fg) = tag_colors(theme, kind);
    let node = (
        Node {
            padding: UiRect::new(units.val(0.4 * k), units.val(0.4 * k), units.val(0.26 * k), units.val(0.2 * k)),
            ..default()
        },
        BackgroundColor(bg),
    );
    (node, tag_label(theme, kit, units, text, size_cq, fg))
}

/// YOU and HOST as one chip split on the slant: YOU's right edge and
/// HOST's left edge lean 12°, overlapping by .27 em.
pub fn spawn_you_host(
    commands: &mut Commands,
    slants: &mut Assets<SlantMaterial>,
    theme: &Theme,
    kit: Option<&KitFonts>,
    units: &UiUnits,
    size_cq: f32,
) -> Entity {
    let em = units.cq(size_cq);
    let pad = |a: f32| Val::Px(units.snap(a * em));
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, ..default() }).id();
    for (i, (kind, text)) in [(TagKind::You, "You"), (TagKind::Host, "Host")].into_iter().enumerate() {
        let (bg, fg) = tag_colors(theme, kind);
        let mat = SlantMaterial::new(theme, bg).edges(i == 1, i == 0);
        let (left, right) = if i == 0 { (0.55, 0.8) } else { (0.8, 0.55) };
        let part = commands
            .spawn((
                MaterialNode(slants.add(mat)),
                Node {
                    padding: UiRect::new(pad(left), pad(right), pad(0.36), pad(0.27)),
                    margin: UiRect::left(if i == 1 { pad(-0.27) } else { Val::ZERO }),
                    ..default()
                },
            ))
            .id();
        let label = commands.spawn(tag_label(theme, kit, units, text, size_cq, fg)).id();
        commands.entity(part).add_child(label);
        commands.entity(row).add_child(part);
    }
    row
}

/// The slot left of a name, `h_cq` square: the robot for a bot (in `tone`),
/// else empty (so names line up).
pub fn bot_slot(
    images: &mut Assets<Image>,
    icons: &mut IconImages,
    theme: &Theme,
    units: &UiUnits,
    bot: bool,
    h_cq: f32,
    tone: Tone,
) -> impl Bundle + use<> {
    let px = units.phys(units.cq(h_cq)).round().max(1.0) as u32;
    let side = Val::Px(px as f32 / units.scale_factor);
    let image = if bot { icons.get(images, "bot", px).unwrap_or_default() } else { Handle::default() };
    let color = if bot { theme.tone(tone) } else { Color::NONE };
    (ImageNode { image, color, ..default() }, Node { width: side, height: side, flex_shrink: 0.0, ..default() })
}

/// Your row: a 7 % light tint and a 1 px outline at 60 % (inset).
pub fn row_you(theme: &Theme, units: &UiUnits) -> impl Bundle + use<> {
    (
        BackgroundColor(theme.tone_a(Tone::Fg, 0.07)),
        Outline::new(Val::Px(units.hairline()), Val::Px(-units.hairline()), theme.tone_a(Tone::Fg, 0.6)),
    )
}

/// A hoverable row's hover mark, a child of the row (`row_h_cq` tall): a
/// slanted tick .22 cq wide at .35 cq from the right edge, 22 % in from
/// the top and the bottom, shown while the row is `Hovered` (`HoverTicks`). Give the
/// row `Hovered::default()` and `position_type: Relative`.
#[derive(Component)]
pub struct HoverTick;

pub fn hover_tick(slants: &mut Assets<SlantMaterial>, theme: &Theme, units: &UiUnits, row_h_cq: f32) -> impl Bundle + use<> {
    // A slanted node is drawn inside its box, so the box is the tick's
    // width plus its lean over its height.
    let h = row_h_cq * 0.56;
    (
        HoverTick,
        MaterialNode(slants.add(SlantMaterial::new(theme, theme.tone(Tone::Fg)))),
        Node {
            position_type: PositionType::Absolute,
            right: units.val(0.35),
            top: units.val(row_h_cq * 0.22),
            width: units.val(0.22 + h * theme.tan()),
            height: units.val(h),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    )
}

/// Shows a row's `HoverTick` while the row is hovered.
pub struct HoverTicks;

impl Plugin for HoverTicks {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_hover_ticks);
    }
}

fn show_hover_ticks(rows: Query<(&Hovered, &Children), Changed<Hovered>>, mut ticks: Query<&mut Visibility, With<HoverTick>>) {
    for (hovered, children) in &rows {
        for c in children.iter() {
            if let Ok(mut v) = ticks.get_mut(c) {
                v.set_if_neq(if hovered.get() { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
}

/// A key cap: outlined, the key in Mono.
pub fn kbd(
    theme: &Theme,
    kit: Option<&KitFonts>,
    units: &UiUnits,
    key: &str,
    size_cq: f32,
) -> (impl Bundle + use<>, impl Bundle + use<>) {
    let s = fonts::text(theme, kit, Role::Mono, units.text_px(size_cq));
    let node = (
        Node {
            padding: UiRect::new(units.val(0.42), units.val(0.42), units.val(0.28), units.val(0.22)),
            border: UiRect::all(Val::Px(units.hairline())),
            ..default()
        },
        BorderColor::all(theme.tone(Tone::Fg3)),
    );
    let label = (Text::new(s.case(key).to_uppercase()), s.bundle(), TextColor(theme.tone(Tone::Fg)));
    (node, label)
}

/// Spawns a (box, text) pair as a parent and child; returns the box.
pub fn spawn_pair(commands: &mut Commands, (node, label): (impl Bundle, impl Bundle)) -> Entity {
    let e = commands.spawn(node).id();
    let t = commands.spawn(label).id();
    commands.entity(e).add_child(t);
    e
}

/// A connection's quality for the ping meter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PingLevel {
    /// Bars lit, 0-4 (0: a bot, no connection).
    pub lit: u32,
    /// The lit bars' and the number's tone.
    pub tone: Tone,
    /// The tone's alpha (display space).
    pub alpha: f32,
}

impl PingLevel {
    /// By the theme's thresholds (`[network]`): quiet (4 bars, `Good` at
    /// `good_alpha`), fine (3, `Fg`), a warning (2, `Warn`), else bad (1,
    /// `Threat`).
    pub fn from_ms(theme: &Theme, ms: u32) -> PingLevel {
        let n = theme.network;
        let (lit, tone, alpha) = if ms < n.ping_quiet_ms {
            (4, Tone::Good, n.good_alpha)
        } else if ms < n.ping_fine_ms {
            (3, Tone::Fg, 1.0)
        } else if ms < n.ping_warn_ms {
            (2, Tone::Warn, 1.0)
        } else {
            (1, Tone::Threat, 1.0)
        };
        PingLevel { lit, tone, alpha }
    }

    /// A bot's: no bars, `Fg4`.
    pub const BOT: PingLevel = PingLevel { lit: 0, tone: Tone::Fg4, alpha: 1.0 };

    /// The lit colour.
    pub fn color(&self, theme: &Theme) -> Color {
        theme.tone_a(self.tone, self.alpha)
    }
}

/// The ping meter, `h_cq` tall (19 x 16 units): four bars 3 units wide,
/// 4, 8, 12, 16 units tall, leaning 12°, `level.lit` lit in its tone, the
/// rest `fg` at 16 %. Returns the meter's node.
pub fn spawn_ping_meter(
    commands: &mut Commands,
    slants: &mut Assets<SlantMaterial>,
    theme: &Theme,
    units: &UiUnits,
    h_cq: f32,
    level: PingLevel,
) -> Entity {
    let unit = units.cq(h_cq) / 16.0;
    let meter = commands
        .spawn(Node { width: Val::Px(units.snap(19.0 * unit)), height: Val::Px(units.snap(16.0 * unit)), ..default() })
        .id();
    for i in 0..4u32 {
        let h = 4.0 * (i + 1) as f32;
        let fill = if i < level.lit { level.color(theme) } else { theme.tone_a(Tone::Fg, 0.16) };
        let bar = commands
            .spawn((
                MaterialNode(slants.add(SlantMaterial::new(theme, fill))),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(units.snap(4.0 * i as f32 * unit)),
                    bottom: Val::Px(0.0),
                    width: Val::Px(units.snap((3.0 + h * theme.tan()) * unit)),
                    height: Val::Px(units.snap(h * unit)),
                    ..default()
                },
            ))
            .id();
        commands.entity(meter).add_child(bar);
    }
    meter
}
