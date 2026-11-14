//! The party menu's first page in the Signal look (docs/plans/active/
//! ugg-ui.md, 3 and M5), for an app whose theme lays the menu out
//! (`[layout.menu]`: the Ultimate Gun Game's); without one
//! `main_menu` draws its VGUI list as before (Lucker Party has no menu).
//! Presentation only: the entries, their actions and the pages are
//! `main_menu`'s (`MenuData`, `MainMenuAction`); Play, Join and Options
//! still open their VGUI frames over this page.
//!
//! On the menu's camera, under its frames (`MENU_Z - 1`), a 16:9 frame
//! (100 x 56.25 cq, centred) over the ambient ground (the kit's
//! `AmbientMaterial`, calm behind the menu column) and the backdrops
//! (`menu_backdrops`):
//!
//! - **Lockup** top left: the kit's `mark`, ULTIMATE (Regular, spread
//!   over the wordmark's width) over GUN GAME (the wordmark: Black, ss02).
//!   It wipes in from the left, ULTIMATE's letters flicker on one by one.
//! - **Menu**: `MenuData`'s entries (index, label, a triangle), stepping
//!   in on the slant one after another; the highlight (a white bar with a
//!   slanted right edge and an accent bar at the left, `SlantMaterial`)
//!   wipes in, then moves between entries (pointer or ↑↓), the entries'
//!   inverted copy clipped to it (`Overflow::clip` on the bar, the copy
//!   moved the other way). Enter or a click acts (`MainMenuAction::Entry`).
//! - **Account** top right: Friends (an inert placeholder) and the
//!   player's name (`NetSettings::name`).
//! - **Keys** along the foot, the backdrop's caption and its tabs (A, B,
//!   C with a dwell bar) bottom right, `+` marks in the corners, the rule
//!   of the game up the right edge.
//! - **Backdrops** (`menu_backdrops`): the next when the shown one's
//!   dwell is up (`rotate`), a tab click shows one from its start.
//!
//! Entrances run on the kit's `Timeline`s from when the page shows; a
//! window resize rebuilds the page where its entrance is. Reduced motion
//! (`ui_reduce_motion 1`): no entrance, no auto-swap, still backdrops.
//! `ui_scene menu [ladder|assembly|ident] [t]` (console) stages it for
//! screenshots: the page at rest, that backdrop frozen `t` s in (`ui_scene
//! menu assembly 2 0.4`: 40 % through the swap into it from the one
//! before, the UI clock stopped);
//! `ui_scene menu enter <t>` the page's entrance `t` s in, the UI clock
//! stopped there (`ui_scene off` runs it again).
//!
//! Known shortcuts: the pages (Play, Join, Options) are still VGUI
//! frames; no text glow (Bevy UI has no blur); the highlight's glow is
//! left out; Friends does nothing.

use bevy::{prelude::*, ui::FocusPolicy};
use chimera_client::client::{logo_pass::ReduceMotion, widgets};
use chimera_ui::{
    AmbientMaterial, Ease, IconImages, KitFonts, Prop, Role, SlantMaterial, Theme, Timeline, Tone, Track, UiClock,
    UiParityId, UiStyle, UiUnits, icons, style,
};

use crate::main_menu::{MENU_Z, MainMenuAction, MenuData, MenuPage, MenuUiCamera, PartyMenu, Screen, ShownScreen};

/// The theme's menu table.
pub const L: &str = "menu";

/// Whether the theme draws the menu here (`[layout.menu]`).
pub fn themed(theme: Option<&Theme>) -> bool {
    theme.is_some_and(|t| t.has_layout(L))
}

fn themed_res(theme: Option<Res<Theme>>) -> bool {
    themed(theme.as_deref())
}

/// The backdrops, in tab order.
pub const BACKDROPS: [(&str, &str, &str); 3] = [
    ("A", "Ladder", "Fourteen rungs, one gun per kill, knife at the top"),
    ("B", "Assembly", "Guns from every game, built line by line"),
    // M5b: the in-engine Showcase takes this slot when assets are at hand.
    ("C", "Ident", "The logo signing on"),
];

/// The page's state: the highlighted entry and the backdrops' rotation.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct SignalMenu {
    /// The highlighted entry (index into `MenuData::entries`).
    pub sel: usize,
    /// The backdrop shown (index into `BACKDROPS`).
    pub backdrop: usize,
    /// The one it swaps from, while the swap runs, and when that one began.
    pub previous: Option<usize>,
    pub previous_since: f64,
    /// When the swap began (`UiClock`).
    pub swap_at: f64,
    /// When the current backdrop began (its own clock's zero).
    pub since: f64,
    /// Swaps by themselves (off with reduced motion or a staged scene).
    pub auto: bool,
    /// Every backdrop held at this many seconds in (screenshots).
    pub freeze: Option<f32>,
    /// When the page last came up (its entrance's zero).
    pub entered: Option<f64>,
    /// A staged scene asked for the page again (its entrance from
    /// `entered`): counts the asks.
    pub staged: u32,
    /// `ui_scene menu enter` stopped the UI clock (`off` starts it).
    pub clock_stopped: bool,
}

impl Default for SignalMenu {
    fn default() -> Self {
        SignalMenu {
            sel: 0,
            backdrop: 0,
            previous: None,
            previous_since: 0.0,
            swap_at: -10.0,
            since: 0.0,
            auto: true,
            freeze: None,
            entered: None,
            staged: 0,
            clock_stopped: false,
        }
    }
}

impl SignalMenu {
    /// How long backdrop `i` stays before the next (the Ladder its whole
    /// climb, `menu_backdrops::LADDER_PERIOD`).
    pub fn dwell(theme: &Theme, i: usize) -> f32 {
        match i {
            0 => crate::menu_backdrops::LADDER_PERIOD,
            _ => theme.dur_opt(L, "backdrop_dwell").unwrap_or(16.0),
        }
    }

    /// Shows backdrop `i` from its start (a tab clicked, the dwell up).
    pub fn choose(&mut self, i: usize, now: f64) {
        if i == self.backdrop {
            self.since = now;
            return;
        }
        self.previous = Some(self.backdrop);
        self.previous_since = self.since;
        self.backdrop = i;
        self.swap_at = now;
        self.since = now;
    }

    /// Seconds into backdrop `i` at `now` (the frozen time if held).
    pub fn local(&self, now: f64) -> f32 {
        self.freeze.unwrap_or((now - self.since) as f32)
    }
}

/// The page's root.
#[derive(Component)]
pub struct SignalMenuRoot;

/// An entry (its index), on the base copy.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalItem(pub usize);

/// The highlight bar (moves to the selected entry) and the inverted copy
/// inside it (moves the other way).
#[derive(Component)]
pub struct SignalHighlight;
#[derive(Component)]
struct SignalInverse;

/// A backdrop tab (its index) and its dwell bar.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalTab(pub usize);
#[derive(Component)]
struct TabDwell(usize);
#[derive(Component)]
struct TabLetter(usize);
#[derive(Component)]
struct TabLabel(usize);
/// The backdrop's caption lines.
#[derive(Component)]
struct Caption(u8);

/// The ambient ground.
#[derive(Component)]
pub struct MenuAmbient;

pub struct SignalMenuPlugin;

impl Plugin for SignalMenuPlugin {
    fn build(&self, app: &mut App) {
        chimera_client::client::signal::setup(app);
        app.init_resource::<SignalMenu>()
            .add_systems(
                Update,
                (build, select, rotate, paint_tabs)
                    .chain()
                    .after(crate::main_menu::MainMenuSystems)
                    .run_if(themed_res),
            )
            .add_systems(
                PostUpdate,
                place_highlight.before(chimera_ui::UiKitSystems).run_if(themed_res),
            );
        crate::menu_backdrops::add(app);
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "menu", stage);
    }
}

/// What the page needs to draw.
struct Kit<'a> {
    theme: &'a Theme,
    fonts: Option<&'a KitFonts>,
    units: UiUnits,
}

impl Kit<'_> {
    fn lay(&self, k: &str, or: f32) -> f32 {
        self.theme.layout_opt(L, k).unwrap_or(or)
    }

    fn dur(&self, k: &str, or: f32) -> f32 {
        self.theme.dur_opt(L, k).unwrap_or(or)
    }

    /// A text in `role` at `size` cq, tracked `track` em.
    fn text(&self, role: Role, size: f32, track: f32, color: Color, text: &str) -> impl Bundle + use<> {
        let px = self.units.text_px(size);
        let mut s = chimera_ui::fonts::text(self.theme, self.fonts, role, px);
        s.spacing = bevy::text::LetterSpacing::Px(track * px);
        (
            Text::new(s.case(text).to_uppercase()),
            s.bundle(),
            TextColor(color),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        )
    }

    fn node(&self, s: chimera_ui::UiStyle) -> Node {
        s.node(&self.units)
    }

    /// The kit's icon `key`, `h` cq tall.
    fn icon(
        &self,
        icons: &mut IconImages,
        images: &mut Assets<Image>,
        key: &str,
        h: f32,
        color: Color,
    ) -> impl Bundle + use<> {
        let u = self.units;
        let px = u.phys(u.cq(h)).round().max(1.0) as u32;
        let aspect = icons.icons().get(key).map_or(1.0, |i| i.aspect());
        let image = icons.get(images, key, px).unwrap_or_default();
        (
            ImageNode {
                image,
                color,
                ..default()
            },
            Node {
                width: Val::Px((aspect * px as f32).ceil() / u.scale_factor),
                height: Val::Px(px as f32 / u.scale_factor),
                flex_shrink: 0.0,
                ..default()
            },
        )
    }

    /// A small right-pointing triangle `h` cq tall.
    fn tri(&self, images: &mut Assets<Image>, h: f32, color: Color) -> impl Bundle + use<> {
        let u = self.units;
        let px = u.phys(u.cq(h)).round().max(1.0) as u32;
        let icon = icons::Icon {
            name: "Triangle".into(),
            w: 46.0,
            h: 56.0,
            shapes: vec![vec![vec![[0.0, 0.0], [46.0, 28.0], [0.0, 56.0]]]],
        };
        (
            ImageNode {
                image: images.add(icons::image(&icon, px)),
                color,
                ..default()
            },
            Node {
                width: Val::Px((px as f32 * 46.0 / 56.0).ceil() / u.scale_factor),
                height: Val::Px(px as f32 / u.scale_factor),
                flex_shrink: 0.0,
                ..default()
            },
        )
    }
}

/// An entrance: opacity 0 to 1 `at` s in over `dur`.
fn fade_in(at: f32, dur: f32, ease: Ease) -> Track {
    Track::new(Prop::Opacity, 0.0, 1.0).at(at).dur(dur).ease(ease)
}

/// What the page was built for.
#[derive(Default, PartialEq)]
struct Built {
    up: bool,
    units: Option<(u32, u32)>,
    entries: Vec<String>,
    name: String,
    still: bool,
    frozen: bool,
    staged: u32,
}

/// Builds the page when it comes up (its entrance from then), again on a
/// resize (at rest), and hides it while another screen shows.
#[allow(clippy::too_many_arguments)]
fn build(
    theme: Res<Theme>,
    fonts: Option<Res<KitFonts>>,
    units: Res<UiUnits>,
    clock: Res<UiClock>,
    shown: Res<ShownScreen>,
    menu: Res<PartyMenu>,
    data: Res<MenuData>,
    net: Option<Res<chimera_net::net::NetSettings>>,
    reduce: Option<Res<ReduceMotion>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    roots: Query<(Entity, Option<&UiTargetCamera>), With<SignalMenuRoot>>,
    (mut state, mut images, mut icon_images, mut slants, mut ambients): (
        ResMut<SignalMenu>,
        Option<ResMut<Assets<Image>>>,
        ResMut<IconImages>,
        Option<ResMut<Assets<SlantMaterial>>>,
        Option<ResMut<Assets<AmbientMaterial>>>,
    ),
    mut built: Local<Built>,
    mut commands: Commands,
) {
    let up = shown.screen == Screen::MainMenu && menu.failure.is_none() && menu.reconnecting.is_none();
    let still = reduce.is_some_and(|r| r.on != 0);
    let now = Built {
        up,
        units: Some(((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32)),
        entries: data.entries.iter().map(|e| e.label.clone()).collect(),
        name: net.as_ref().map_or_else(String::new, |n| n.name.clone()),
        still,
        frozen: state.freeze.is_some(),
        staged: state.staged,
    };
    // Keep the root on the menu's camera.
    if let Some(cam) = camera.iter().next() {
        for (r, on) in &roots {
            if on.is_none_or(|t| t.0 != cam) {
                commands.entity(r).insert(UiTargetCamera(cam));
            }
        }
    }
    if *built == now && !theme.is_changed() {
        return;
    }
    let came_up = up && !built.up;
    let restaged = built.staged != now.staged;
    *built = now;
    for (r, _) in &roots {
        commands.entity(r).despawn();
    }
    if !up {
        state.entered = None;
        return;
    }
    let (Some(images), Some(slants)) = (images.as_deref_mut(), slants.as_deref_mut()) else {
        return;
    };
    if (came_up && !restaged) || state.entered.is_none() {
        state.entered = Some(clock.now);
        state.since = clock.now;
        state.previous = None;
    }
    // No auto swap with reduced motion or a staged scene.
    state.auto = state.freeze.is_none() && !still;
    // A rebuild that isn't the page coming up shows it at rest.
    // From the entrance's start (a resize mid-way carries on; reduced
    // motion: at rest).
    let start = match state.entered {
        Some(at) if !still => at,
        _ => clock.now - 60.0,
    };
    if state.sel >= data.entries.len() {
        state.sel = 0;
    }
    let k = Kit {
        theme: &theme,
        fonts: fonts.as_deref(),
        units: *units,
    };
    spawn_page(
        &mut commands,
        &k,
        &data,
        &state,
        &built.name,
        start,
        images,
        &mut icon_images,
        slants,
        ambients.as_deref_mut(),
        camera.iter().next(),
        still || state.freeze.is_some(),
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_page(
    c: &mut Commands,
    k: &Kit,
    data: &MenuData,
    state: &SignalMenu,
    name: &str,
    start: f64,
    images: &mut Assets<Image>,
    icon_images: &mut IconImages,
    slants: &mut Assets<SlantMaterial>,
    ambients: Option<&mut Assets<AmbientMaterial>>,
    camera: Option<Entity>,
    still: bool,
) {
    let t = k.theme;
    let u = k.units;
    let hair = u.hairline();
    let out = t.ease("out");
    let mut root = c.spawn((
        SignalMenuRoot,
        Name::new("Signal menu"),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.0),
            height: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(t.tone(Tone::Ink)),
        GlobalZIndex(MENU_Z - 1),
        FocusPolicy::Pass,
    ));
    if let Some(cam) = camera {
        root.insert(UiTargetCamera(cam));
    }
    let root = root.id();
    // The ground: the ambient under everything, the backdrops over it.
    if let Some(ambients) = ambients {
        c.spawn((
            MenuAmbient,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            MaterialNode(ambients.add({
                let (column, bloom) = AmbientMaterial::MENU;
                // Still with reduced motion and in a staged scene.
                AmbientMaterial::new(t)
                    .column(Some(k.lay("calm", column)))
                    .bloom(bloom)
                    .freeze(still.then_some(40.0))
            })),
            ChildOf(root),
        ));
    }
    crate::menu_backdrops::spawn_layer(c, root);

    let frame = c
        .spawn((k.node(style!(row, w: 100.0, h: 56.25).shrink(0.0)), ChildOf(root)))
        .id();

    // Corners.
    let corner = k.lay("corner", 2.4);
    for (x, y) in [
        (corner, corner),
        (100.0 - corner, corner),
        (corner, 56.25 - corner),
        (100.0 - corner, 56.25 - corner),
    ] {
        let s = 1.1;
        let plus = c
            .spawn((
                k.node(style!(row, w: s, h: s).abs([Some(x - s / 2.0), Some(y - s / 2.0), None, None])),
                ChildOf(frame),
            ))
            .id();
        let line = t.tone(Tone::Fg3);
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                width: px(hair),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(line),
            ChildOf(plus),
        ));
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(50.0),
                height: px(hair),
                width: percent(100.0),
                ..default()
            },
            BackgroundColor(line),
            ChildOf(plus),
        ));
    }

    // The lockup: wipes in from the left (a clip that slides in while its
    // content holds still), ULTIMATE's letters flickering on.
    let lk = k.lay("lockup", 3.3);
    let lk_w = k.lay("lockup_w", 34.0);
    let (lk_at, lk_dur) = (k.dur("lockup_at", 0.12), k.dur("lockup", 0.6));
    let clip = c
        .spawn((
            clipped(k.node(style!(row, w: lk_w).abs([
                Some(k.lay("lockup_x", 4.6)),
                Some(k.lay("lockup_y", 5.6)),
                None,
                None,
            ]))),
            UiTransform::default(),
            Timeline::new([Track::new(Prop::X, -lk_w, 0.0).at(lk_at).dur(lk_dur).ease(out)]).started(start),
            ChildOf(frame),
        ))
        .id();
    let lockup = c
        .spawn((
            k.node(style!(row, gap: 0.55 * lk).shrink(0.0)),
            UiTransform::default(),
            Timeline::new([Track::new(Prop::X, lk_w, 0.0).at(lk_at).dur(lk_dur).ease(out)]).started(start),
            UiParityId::new("menu.lockup"),
            ChildOf(clip),
        ))
        .id();
    let mark = c
        .spawn((
            k.icon(icon_images, images, "mark", 2.05 * lk, t.tone(Tone::Fg)),
            UiParityId::new("menu.lockup.mark"),
            ChildOf(lockup),
        ))
        .id();
    let _ = mark;
    let words = c.spawn((k.node(style!(column, gap: 0.3 * lk)), ChildOf(lockup))).id();
    let ult = c
        .spawn((
            k.node(style!(row).justify(JustifyContent::SpaceBetween)),
            UiParityId::new("menu.lockup.ult"),
            ChildOf(words),
        ))
        .id();
    let (ult_at, ult_gap, ult_dur) = (k.dur("ult_at", 0.38), k.dur("ult_gap", 0.045), k.dur("ult", 0.2));
    for (j, ch) in "ULTIMATE".chars().enumerate() {
        c.spawn((
            k.text(
                Role::DisplayRegular,
                0.62 * lk,
                0.0,
                t.tone_a(Tone::Fg, t.alpha("fg_2").unwrap_or(0.66)),
                &ch.to_string(),
            ),
            Timeline::new([fade_in(ult_at + j as f32 * ult_gap, ult_dur, Ease::Steps(2))]).started(start),
            ChildOf(ult),
        ));
    }
    c.spawn((
        k.text(Role::Wordmark, lk, 0.03, t.tone(Tone::Fg), "GUN GAME"),
        UiParityId::new("menu.lockup.gg"),
        ChildOf(words),
    ));

    // The account chip.
    let fade = k.dur("fade", 0.5);
    let acct = c
        .spawn((
            k.node(style!(row, gap: 0.45).abs([None, Some(k.lay("acct_y", 5.6)), Some(k.lay("acct_x", 4.6)), None])),
            UiTransform::default(),
            Timeline::new([fade_in(k.dur("acct_at", 0.7), fade, out)]).started(start),
            UiParityId::new("menu.acct"),
            ChildOf(frame),
        ))
        .id();
    let tab_text = k.lay("tab_text", 0.64);
    for (label, inert, id) in [
        ("Friends \u{2022} 3", true, "menu.acct.friends"),
        (name, false, "menu.acct.name"),
    ] {
        let label = if label.is_empty() { "Player" } else { label };
        let (bg, fg) = if inert {
            (t.tone_a(Tone::Ink, 0.9), t.tone(Tone::Fg))
        } else {
            (t.tone(Tone::Fg), t.tone(Tone::Ink))
        };
        let dim = if inert { 0.45 } else { 1.0 };
        let mut node =
            k.node(style!(row, gap: 1.6, pad: [0.5, 0.6, 0.45, 0.7], min_w: 7.4).justify(JustifyContent::SpaceBetween));
        if inert {
            node.border = UiRect::all(px(hair));
        }
        let mut tab = c.spawn((
            node,
            BackgroundColor(bg.with_alpha(bg.alpha() * dim)),
            UiParityId::new(id),
            ChildOf(acct),
        ));
        if inert {
            // A border, not an outline: the fade dims borders.
            tab.insert(BorderColor::all(t.tone_a(Tone::Fg, 0.42 * dim)));
        }
        let tab = tab.id();
        c.spawn((
            k.text(Role::Mono, tab_text, 0.1, fg.with_alpha(dim), label),
            ChildOf(tab),
        ));
        c.spawn((k.tri(images, 0.8 * 0.56, fg.with_alpha(dim)), ChildOf(tab)));
    }

    // The menu: the entries, the highlight, the inverted copy in it.
    let (mx, my, mw, ih) = (
        k.lay("items_x", 4.6),
        k.lay("items_y", 23.4),
        k.lay("items_w", 24.0),
        k.lay("item_h", 3.5),
    );
    let nav = c
        .spawn((
            k.node(style!(column, w: mw).abs([Some(mx), Some(my), None, None])),
            ChildOf(frame),
        ))
        .id();
    let (items_at, item_dur, item_gap) = (k.dur("items_at", 0.24), k.dur("item", 0.52), k.dur("item_gap", 0.055));
    let entry_row =
        |c: &mut Commands, images: &mut Assets<Image>, parent: Entity, i: usize, label: &str, inverse: bool| {
            let color = |tone: Tone, a: f32| if inverse { t.tone(Tone::Ink) } else { t.tone_a(tone, a) };
            let at = items_at + i as f32 * item_gap;
            // The step in: the row slides in from the left on the slant while
            // a clip reveals it left to right (the clip slides, its content
            // holds), fading in.
            let dx = -(2.4 + i as f32 * 0.62);
            let row = c
                .spawn((
                    clipped(k.node(style!(row, w: mw, h: ih).shrink(0.0))),
                    UiTransform::default(),
                    Timeline::new([
                        Track::new(Prop::X, dx - mw, 0.0).at(at).dur(item_dur).ease(out),
                        fade_in(at, item_dur, out),
                    ])
                    .started(start),
                    ChildOf(parent),
                ))
                .id();
            let li = c
                .spawn((
                    k.node(
                        style!(row, gap: k.lay("item_gap", 1.1), pad: [0.0, 1.5, 0.0, 1.1], w: mw, h: ih).shrink(0.0),
                    ),
                    UiTransform::default(),
                    Timeline::new([Track::new(Prop::X, mw, 0.0).at(at).dur(item_dur).ease(out)]).started(start),
                    ChildOf(row),
                ))
                .id();
            if !inverse {
                c.entity(row).insert((
                    SignalItem(i),
                    MainMenuAction::Entry(i),
                    widgets::clickable(),
                    UiParityId::new(format!("menu.item.{}", i + 1)),
                ));
                if i > 0 {
                    c.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: k.units.val(1.1),
                            right: px(0.0),
                            top: px(0.0),
                            height: px(hair),
                            ..default()
                        },
                        BackgroundColor(t.tone_a(Tone::Fg, t.alpha("fg_5").unwrap_or(0.07))),
                        ChildOf(li),
                    ));
                }
            }
            let ix = c
                .spawn((k.node(style!(row, w: k.lay("item_ix_w", 1.3)).shrink(0.0)), ChildOf(li)))
                .id();
            c.spawn((
                k.text(
                    Role::Mono,
                    k.lay("item_ix", 0.62),
                    0.1,
                    color(Tone::Fg, 0.42),
                    &format!("{:02}", i + 1),
                ),
                ChildOf(ix),
            ));
            let lab = c
                .spawn((
                    k.text(
                        Role::DisplayBold,
                        k.lay("item_label", 1.36),
                        0.06,
                        color(Tone::Fg, 1.0),
                        label,
                    ),
                    ChildOf(li),
                ))
                .id();
            if !inverse {
                c.entity(lab)
                    .insert(UiParityId::new(format!("menu.item.{}.label", i + 1)));
            }
            c.spawn((k.node(style!(row).grow(1.0)), ChildOf(li)));
            c.spawn((k.tri(images, 0.9 * 0.56, color(Tone::Fg, 0.42)), ChildOf(li)));
        };
    for (i, e) in data.entries.iter().enumerate() {
        entry_row(c, images, nav, i, &e.label, false);
    }
    // The highlight: wipes in from the left, then follows the selection.
    let (bar_at, bar_dur) = (k.dur("bar_at", 0.62), k.dur("bar", 0.45));
    let y0 = state.sel as f32 * ih;
    let hl = c
        .spawn((
            SignalHighlight,
            clipped(k.node(style!(row, w: mw, h: ih).abs([Some(0.0), Some(0.0), None, None]))),
            UiTransform::default(),
            Timeline::new([
                Track::new(Prop::X, -mw, 0.0).at(bar_at).dur(bar_dur).ease(out),
                Track::new(Prop::Y, y0, y0),
            ])
            .started(start),
            UiParityId::new("menu.hl"),
            ChildOf(nav),
        ))
        .id();
    let slant = t.tan() * ih;
    let slab = SlantMaterial::new(t, t.tone(Tone::Fg))
        .edges(false, true)
        .accent(t.tone(Tone::Acc), u.phys(u.cq(k.lay("hl_accent", 0.32))).round());
    let _ = slant;
    c.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: k.units.val(mw),
            height: k.units.val(ih),
            ..default()
        },
        MaterialNode(slants.add(slab)),
        UiTransform::default(),
        Timeline::new([Track::new(Prop::X, mw, 0.0).at(bar_at).dur(bar_dur).ease(out)]).started(start),
        ChildOf(hl),
    ));
    let inv = c
        .spawn((
            SignalInverse,
            k.node(style!(column, w: mw).abs([Some(0.0), Some(0.0), None, None])),
            UiTransform::default(),
            Timeline::new([
                Track::new(Prop::X, mw, 0.0).at(bar_at).dur(bar_dur).ease(out),
                Track::new(Prop::Y, -y0, -y0),
                fade_in(k.dur("inv_at", 0.9), 0.01, Ease::Linear),
            ])
            .started(start),
            ChildOf(hl),
        ))
        .id();
    for (i, e) in data.entries.iter().enumerate() {
        entry_row(c, images, inv, i, &e.label, true);
    }

    // The keys along the foot.
    let keys_at = k.dur("keys_at", 0.9);
    let keys = c
        .spawn((
            k.node(style!(row, gap: k.lay("keys_gap", 2.2)).abs([Some(mx), None, None, Some(k.lay("keys_y", 5.2))])),
            UiTransform::default(),
            Timeline::new([fade_in(keys_at, fade, out)]).started(start),
            UiParityId::new("menu.keys"),
            ChildOf(frame),
        ))
        .id();
    let ks = k.lay("keys", 0.62);
    for (i, (key, what)) in [("\u{2191}\u{2193}", "Select"), ("Enter", "Confirm"), ("Esc", "Back")]
        .into_iter()
        .enumerate()
    {
        let pair = c
            .spawn((
                k.node(style!(row, gap: 0.55)),
                UiParityId::new(format!("menu.keys.{}", i + 1)),
                ChildOf(keys),
            ))
            .id();
        let (node, label) = chimera_ui::widgets::kbd(t, k.fonts, &u, key, ks);
        let cap = widgets_pair(c, (node, label));
        c.entity(cap).insert(ChildOf(pair));
        c.spawn((
            k.text(
                Role::Mono,
                ks,
                0.1,
                t.tone_a(Tone::Fg, t.alpha("fg_2").unwrap_or(0.66)),
                what,
            ),
            ChildOf(pair),
        ));
    }

    // The game's rule up the right edge: a row turned a quarter, centred
    // in a column as wide as its line.
    let vs = k.lay("vt", 0.56);
    let vt = c
        .spawn((
            k.node(
                UiStyle {
                    h_pct: Some(100.0),
                    ..style!(column, w: vs).abs([None, Some(0.0), Some(k.lay("vt_x", 2.1)), None])
                }
                .justify(JustifyContent::Center)
                .align(AlignItems::Center),
            ),
            UiTransform::default(),
            Timeline::new([fade_in(keys_at, fade, out)]).started(start),
            ChildOf(frame),
        ))
        .id();
    let vline = c
        .spawn((
            k.node(style!(row, gap: 1.0).shrink(0.0)),
            UiTransform {
                rotation: Rot2::degrees(90.0),
                ..default()
            },
            ChildOf(vt),
        ))
        .id();
    for w in [
        "Every kill",
        "\u{25B8}",
        "new gun",
        "\u{25B8}",
        "14 rungs",
        "\u{25B8}",
        "knife wins",
    ] {
        c.spawn((k.text(Role::Mono, vs, 0.3, t.tone_a(Tone::Fg, 0.42), w), ChildOf(vline)));
    }

    // The backdrop's caption and tabs.
    let cap = c
        .spawn((
            k.node(
                style!(column, gap: 0.2, max_w: 22.0)
                    .abs([None, None, Some(k.lay("tabs_x", 4.6)), Some(k.lay("cap_y", 7.6))])
                    .align(AlignItems::FlexEnd),
            ),
            UiTransform::default(),
            Timeline::new([fade_in(keys_at, fade, out)]).started(start),
            UiParityId::new("menu.cap"),
            ChildOf(frame),
        ))
        .id();
    let cs = k.lay("cap", 0.56);
    let (a, n, line) = BACKDROPS[state.backdrop.min(2)];
    c.spawn((
        k.text(
            Role::Mono,
            cs,
            0.12,
            t.tone_a(Tone::Fg, 0.66),
            &format!("{a} \u{2022} {n}"),
        ),
        Caption(0),
        ChildOf(cap),
    ));
    c.spawn((
        k.text(Role::Mono, cs, 0.12, t.tone_a(Tone::Fg, 0.42), line),
        Caption(1),
        ChildOf(cap),
    ));
    let tabs = c
        .spawn((
            k.node(style!(row).abs([None, None, Some(k.lay("tabs_x", 4.6)), Some(k.lay("tabs_y", 4.2))])),
            BackgroundColor(t.tone_a(Tone::Ink, 0.7)),
            Outline::new(px(hair), px(-hair), t.tone_a(Tone::Fg, 0.15)),
            UiParityId::new("menu.tabs"),
            ChildOf(frame),
        ))
        .id();
    let tsz = k.lay("tabs_text", 0.62);
    let lb = c.spawn((k.node(style!(row, pad: [0.0, 0.8])), ChildOf(tabs))).id();
    c.spawn((
        k.text(
            Role::Mono,
            k.lay("tabs_label", 0.56),
            0.12,
            t.tone_a(Tone::Fg, 0.42),
            "Backdrop",
        ),
        ChildOf(lb),
    ));
    for (i, (letter, label, _)) in BACKDROPS.iter().enumerate() {
        let b = c
            .spawn((
                SignalTab(i),
                k.node(style!(row, gap: 0.5, pad: [0.5, 0.75])),
                BackgroundColor(Color::NONE),
                widgets::clickable(),
                UiParityId::new(format!("menu.tab.{}", i + 1)),
                ChildOf(tabs),
            ))
            .id();
        c.spawn((
            k.text(Role::Mono, tsz, 0.1, t.tone_a(Tone::Fg, 0.42), letter),
            TabLetter(i),
            ChildOf(b),
        ));
        c.spawn((
            k.text(Role::Mono, tsz, 0.1, t.tone_a(Tone::Fg, 0.66), label),
            TabLabel(i),
            ChildOf(b),
        ));
        c.spawn((
            TabDwell(i),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                bottom: px(0.0),
                height: px(u.snap(2.0 * u.scale())),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Acc)),
            UiTransform {
                scale: Vec2::new(0.0, 1.0),
                ..default()
            },
            ChildOf(b),
        ));
    }
}

/// `n` clipping what overflows it.
fn clipped(mut n: Node) -> Node {
    n.overflow = Overflow::clip();
    n
}

/// A key cap from the kit's builder pair.
fn widgets_pair(c: &mut Commands, pair: (impl Bundle, impl Bundle)) -> Entity {
    chimera_ui::widgets::spawn_pair(c, pair)
}

/// The selection: the entry under the pointer, ↑↓, Enter acts.
#[allow(clippy::too_many_arguments)]
fn select(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    menu: Res<PartyMenu>,
    shown: Res<ShownScreen>,
    data: Res<MenuData>,
    mut state: ResMut<SignalMenu>,
    items: Query<(&SignalItem, &Interaction), Changed<Interaction>>,
    tabs: Query<(&SignalTab, &Interaction), Changed<Interaction>>,
    clock: Res<UiClock>,
    mut out: MessageWriter<MainMenuAction>,
) {
    let live = shown.screen == Screen::MainMenu
        && menu.page == MenuPage::Main
        && menu.failure.is_none()
        && menu.reconnecting.is_none()
        && !console.is_some_and(|c| c.open);
    if !live {
        return;
    }
    let n = data.entries.len().max(1);
    for (item, i) in &items {
        if *i != Interaction::None && state.sel != item.0 {
            state.sel = item.0;
        }
    }
    for (tab, i) in &tabs {
        if *i == Interaction::Pressed {
            state.choose(tab.0, clock.now);
        }
    }
    let Some(keys) = keys else { return };
    if keys.just_pressed(KeyCode::ArrowDown) {
        state.sel = (state.sel + 1) % n;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        state.sel = (state.sel + n - 1) % n;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) && state.sel < data.entries.len() {
        out.write(MainMenuAction::Entry(state.sel));
    }
}

/// The highlight to the selected entry (and the inverted copy the other
/// way), from wherever it is.
#[allow(clippy::type_complexity)]
fn place_highlight(
    state: Res<SignalMenu>,
    theme: Res<Theme>,
    clock: Res<UiClock>,
    mut hl: Query<&mut Timeline, (With<SignalHighlight>, Without<SignalInverse>)>,
    mut inv: Query<&mut Timeline, (With<SignalInverse>, Without<SignalHighlight>)>,
) {
    let ih = theme.layout_opt(L, "item_h").unwrap_or(3.5);
    let dur = theme.dur_opt(L, "hl_move").unwrap_or(0.3);
    let to = state.sel as f32 * ih;
    for mut tl in &mut hl {
        let last_to = tl.tracks.iter().filter(|t| t.prop == Prop::Y).map(|t| t.to).last();
        if last_to.is_some_and(|l| (l - to).abs() > 1e-4) {
            tl.redirect(clock.now, Prop::Y, to, dur, theme.ease("out"));
        }
    }
    for mut tl in &mut inv {
        let last_to = tl.tracks.iter().filter(|t| t.prop == Prop::Y).map(|t| t.to).last();
        if last_to.is_some_and(|l| (l + to).abs() > 1e-4) {
            tl.redirect(clock.now, Prop::Y, -to, dur, theme.ease("out"));
        }
    }
}

/// The backdrops' rotation: the next when the dwell is up (auto), the
/// swap's end.
fn rotate(theme: Res<Theme>, clock: Res<UiClock>, shown: Res<ShownScreen>, mut state: ResMut<SignalMenu>) {
    if shown.screen != Screen::MainMenu {
        return;
    }
    let swap = theme.dur_opt(L, "backdrop_swap").unwrap_or(0.52) as f64;
    if state.previous.is_some() && clock.now - state.swap_at >= swap {
        state.previous = None;
    }
    if state.auto
        && state.freeze.is_none()
        && (clock.now - state.since) as f32 >= SignalMenu::dwell(&theme, state.backdrop)
    {
        let next = (state.backdrop + 1) % BACKDROPS.len();
        state.choose(next, clock.now);
    }
}

/// The tabs: the shown one white with its letter in the accent and its
/// dwell bar filling; the caption.
#[allow(clippy::type_complexity)]
fn paint_tabs(
    theme: Res<Theme>,
    clock: Res<UiClock>,
    state: Res<SignalMenu>,
    mut tabs: Query<(&SignalTab, &mut BackgroundColor)>,
    mut letters: Query<(&TabLetter, &mut TextColor), Without<TabLabel>>,
    mut labels: Query<(&TabLabel, &mut TextColor), Without<TabLetter>>,
    mut dwell: Query<(&TabDwell, &mut UiTransform)>,
    mut caption: Query<(&Caption, &mut Text)>,
) {
    let t = &*theme;
    for (tab, mut bg) in &mut tabs {
        let on = tab.0 == state.backdrop;
        bg.set_if_neq(BackgroundColor(if on { t.tone(Tone::Fg) } else { Color::NONE }));
    }
    for (l, mut c) in &mut letters {
        let on = l.0 == state.backdrop;
        c.set_if_neq(TextColor(if on {
            t.tone(Tone::Acc)
        } else {
            t.tone_a(Tone::Fg, 0.42)
        }));
    }
    for (l, mut c) in &mut labels {
        let on = l.0 == state.backdrop;
        c.set_if_neq(TextColor(if on {
            t.tone(Tone::Ink)
        } else {
            t.tone_a(Tone::Fg, 0.66)
        }));
    }
    let p = if state.auto && state.freeze.is_none() {
        ((clock.now - state.since) as f32 / SignalMenu::dwell(t, state.backdrop)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for (d, mut tf) in &mut dwell {
        let s = if d.0 == state.backdrop { p } else { 0.0 };
        // Scaled about its left end: the centre moves by half the loss.
        let want = UiTransform {
            scale: Vec2::new(s, 1.0),
            translation: Val2::percent(-(1.0 - s) * 50.0, 0.0),
            ..default()
        };
        if *tf != want {
            *tf = want;
        }
    }
    let (a, n, line) = BACKDROPS[state.backdrop.min(BACKDROPS.len() - 1)];
    for (cap, mut text) in &mut caption {
        let want = if cap.0 == 0 {
            format!("{a} \u{2022} {n}").to_uppercase()
        } else {
            line.to_uppercase()
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}

/// `ui_scene menu [ladder|assembly|ident] [t]`: the menu's first page at
/// rest, that backdrop held `t` s in (screenshots; `off`: moving again).
pub fn stage(world: &mut World, args: &[String]) -> Result<(), String> {
    if args.first().is_some_and(|a| a == "off") {
        let mut s = world.resource_mut::<SignalMenu>();
        s.freeze = None;
        s.auto = true;
        if std::mem::take(&mut s.clock_stopped) {
            world.resource_mut::<UiClock>().manual = false;
        }
        return Ok(());
    }
    // `enter <t>`: the page's entrance `t` s in, the UI clock stopped
    // there (screenshots of the entrance).
    if args.first().is_some_and(|a| a == "enter") {
        let t = args.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.5);
        show_menu(world);
        let mut clock = world.resource_mut::<UiClock>();
        clock.manual = true;
        let now = clock.now;
        let mut s = world.resource_mut::<SignalMenu>();
        s.entered = Some(now - t);
        s.since = now - t;
        s.freeze = Some(t as f32);
        s.staged += 1;
        s.clock_stopped = true;
        return Ok(());
    }
    let which = match args.first().map(|s| s.to_ascii_lowercase()) {
        None => 0,
        Some(a) => match a.as_str() {
            "a" | "ladder" => 0,
            "b" | "assembly" => 1,
            "c" | "ident" | "showcase" => 2,
            a if a.ends_with(".json") => 0,
            other => {
                return Err(format!(
                    "ui_scene menu: no backdrop `{other}` (ladder, assembly, ident)"
                ));
            }
        },
    };
    let t = args.get(1).and_then(|s| s.parse::<f32>().ok());
    // A third number: the swap into it this far through (0-1), from the
    // backdrop before it, the UI clock stopped there.
    let wipe = args.get(2).and_then(|s| s.parse::<f64>().ok());
    show_menu(world);
    let now = world.resource::<UiClock>().now;
    let mut s = world.resource_mut::<SignalMenu>();
    s.backdrop = which;
    s.previous = None;
    s.since = now;
    s.auto = false;
    s.freeze = Some(t.unwrap_or(crate::menu_backdrops::still(which)));
    // At rest: the entrance long over.
    s.entered = Some(now - 60.0);
    s.staged += 1;
    if let Some(w) = wipe {
        let swap = world.get_resource::<Theme>().and_then(|t| t.dur_opt(L, "backdrop_swap")).unwrap_or(0.52) as f64;
        let mut s = world.resource_mut::<SignalMenu>();
        s.previous = Some((which + BACKDROPS.len() - 1) % BACKDROPS.len());
        s.swap_at = now - w.clamp(0.0, 1.0) * swap;
        s.clock_stopped = true;
        world.resource_mut::<UiClock>().manual = true;
    }
    Ok(())
}

/// The menu's first page shown at once (no pass).
fn show_menu(world: &mut World) {
    let mut m = world.resource_mut::<PartyMenu>();
    m.screen = Screen::MainMenu;
    m.page = MenuPage::Main;
    m.failure = None;
    m.loading = None;
    let snapshot = ShownScreen::of(world.resource::<PartyMenu>());
    world.insert_resource(snapshot);
}
