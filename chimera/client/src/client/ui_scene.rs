//! `ui_scene <name>` (console): stages a screen of the UI kit
//! (`chimera_ui`) with made-up data for screenshots
//! (docs/plans/active/ugg-ui.md, section 5); `ui_scene off` clears it.
//! Scenes: `chip`, `lead` (the race chip, chasing or leading), `board`,
//! `board_lead` (the Tab board open), `feed` (five kills in the feed),
//! `rankup` (the rank-up toast's loop on the chip: a roll, then a chain;
//! `rankup <t>` frozen `t` s into the loop, the UI clock stopped until
//! `ui_scene off`), `airstrike` (the effects' loop: moon gravity's chip,
//! an air strike's banner, edge pulses and fold into the chip stack;
//! `airstrike <t>` frozen likewise):
//! the HUD's own screens with made-up players (`race_chip::stage`,
//! `kill_feed::signal_feed::stage`), and `matchend` (the match end with
//! eight players, the climb and the vote: `match_end_signal::stage`),
//! under the Signal theme; given a
//! fixture (`ui_scene board <fixture.json>`), its data over a flat ground
//! instead (the UI parity check, `ui_parity`). `proof` (the kit itself: the type ramp of every role, tick
//! bars, slants, the gun icons and marks, the logo pass frozen mid-way,
//! the ambient and the match end's lines behind). The screens' milestones add theirs
//! (`lobby`, `chip`, `board`, `toast`, `airstrike`, `matchend`, `menu`)
//! to `SCENES`. An app's own screens register theirs in `UiSceneHooks`
//! (`menu`: the party menu's first page, `party_kit::signal_menu::stage`).
//!
//! A scene is a root of its own over everything (`UI_SCENE_Z`), on the
//! default UI camera, laid out for the window's size at the time it is
//! staged, again whenever that changes (`chimera_ui::UiUnits`), from the app's `Theme` and faces
//! (`KitFonts`; the shell's default theme without them).
//!
//! `+ui_scene proof` on the command line stages it at start, e.g.
//! `ultimate_gun_game --offscreen 1920x1080 --frames 30 --screenshot
//! <out>.png +ui_scene proof`.

use bevy::prelude::*;
use chimera_core::console::ConsoleAppExt;
use chimera_ui::{
    Ease, IconImages, KitFonts, LinesMaterial, PassMaterial, Role, SlantMaterial, Theme, TickBarMaterial, TickLayout,
    Tone, UiUnits, fonts, icons,
    widgets::{self, PingLevel, TagKind},
};

/// Above every overlay and menu.
pub const UI_SCENE_Z: i32 = 900;

/// The scenes `ui_scene` stages.
pub const SCENES: &[&str] = &["proof", "chip", "lead", "board", "board_lead", "feed", "rankup", "airstrike", "matchend"];

/// The scene asked for (`None`: none).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct UiScene {
    pub want: Option<String>,
    shown: Option<String>,
    /// A scene froze the UI clock (`ui_scene <name> <t>`): it runs again
    /// at `off`.
    froze: bool,
}

/// Where a scene's loop starts on the UI clock: now, or (`t` given) `t`
/// s ago with the clock frozen there (a screenshot of one moment).
fn scene_start(world: &mut World, t: Option<f64>) -> f64 {
    let mut clock = world.get_resource_or_init::<chimera_ui::UiClock>();
    match t {
        Some(t) => {
            clock.manual = true;
            let now = clock.now;
            world.resource_mut::<UiScene>().froze = true;
            now - t
        }
        None => clock.now,
    }
}

/// Every staged HUD scene cleared, the clock running again if a scene
/// froze it.
fn clear_staged(w: &mut World) {
    super::race_chip::stage(w, None);
    super::rank_toast::stage(w, None);
    super::match_effects::stage(w, None);
    super::match_end_signal::unstage(w);
    if std::mem::take(&mut w.resource_mut::<UiScene>().froze)
        && let Some(mut c) = w.get_resource_mut::<chimera_ui::UiClock>()
    {
        c.manual = false;
    }
}

/// A staged scene's root.
#[derive(Component)]
pub struct UiSceneRoot;

/// An app's scene: stages it from the arguments after its name (`off`
/// when every scene is cleared).
pub type SceneHook = fn(&mut World, &[String]) -> Result<(), String>;

/// Scenes of screens an app draws itself (the party menu's `menu`), by
/// name: `ui_scene <name> ...` calls the hook.
#[derive(Resource, Default, Clone)]
pub struct UiSceneHooks(pub Vec<(&'static str, SceneHook)>);

impl UiSceneHooks {
    /// Adds (or replaces) `name`'s hook.
    pub fn add(world: &mut World, name: &'static str, hook: SceneHook) {
        let mut h = world.get_resource_or_init::<UiSceneHooks>();
        h.0.retain(|(n, _)| *n != name);
        h.0.push((name, hook));
    }
}

pub struct UiScenePlugin;

impl Plugin for UiScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiScene>()
            .console_command(
                "ui_scene",
                "ui_scene <proof|chip|lead|board|board_lead|feed|rankup|airstrike|matchend|off> [fixture.json | t]: stage a screen of the UI \
                 kit with made-up data (screenshots), or a fixture's (the UI parity check); an animated scene (rankup, airstrike) \
                 with `t` frozen `t` s into its loop.",
                |w, a| {
                    let name = a.first().map(String::as_str).unwrap_or("proof");
                    // An animated scene's frozen time.
                    let t = || a.get(1).map(|t| t.parse::<f64>().map_err(|_| format!("ui_scene: not a time: {t}"))).transpose();
                    if name == "off" {
                        w.resource_mut::<UiScene>().want = None;
                        let hooks: Vec<SceneHook> = w.get_resource::<UiSceneHooks>().map(|h| h.0.iter().map(|(_, f)| *f).collect()).unwrap_or_default();
                        for hook in hooks {
                            hook(w, &["off".to_string()])?;
                        }
                        clear_staged(w);
                        super::ui_parity::clear_ground(w);
                        if w.remove_resource::<super::kill_feed::signal_feed::FeedStaged>().is_some()
                            && let Some(mut f) = w.get_resource_mut::<super::kill_feed::KillFeed>()
                        {
                            f.lifetime = super::kill_feed::LIFETIME;
                            f.entries.clear();
                        }
                        return Ok(None);
                    }
                    // An app's own screen (`UiSceneHooks`: the party menu's).
                    let hook = w.get_resource::<UiSceneHooks>().and_then(|h| h.0.iter().find(|(n, _)| *n == name).map(|(_, f)| *f));
                    if let Some(hook) = hook {
                        clear_staged(w);
                        hook(w, &a[1..])?;
                        return Ok(None);
                    }
                    if !SCENES.contains(&name) {
                        let apps: Vec<&str> = w.get_resource::<UiSceneHooks>().map(|h| h.0.iter().map(|(n, _)| *n).collect()).unwrap_or_default();
                        return Err(format!("ui_scene: no scene `{name}` (scenes: {}, {})", SCENES.join(", "), apps.join(", ")));
                    }
                    // The HUD's own screens, staged with made-up data where
                    // they draw (the race chip and board, the kill feed).
                    clear_staged(w);
                    match name {
                        "rankup" => {
                            let start = scene_start(w, t()?);
                            super::rank_toast::stage(w, Some(start));
                            return Ok(None);
                        }
                        "airstrike" => {
                            // On the chip, as in play.
                            let start = scene_start(w, t()?);
                            super::race_chip::stage(w, Some("chip"));
                            super::match_effects::stage(w, Some(start));
                            return Ok(None);
                        }
                        "chip" | "lead" | "board" | "board_lead" => {
                            super::race_chip::stage(w, Some(name));
                            if let Some(path) = a.get(1) {
                                super::ui_parity::apply_fixture(w, name, path)?;
                            }
                            return Ok(None);
                        }
                        "matchend" => {
                            return super::match_end_signal::stage(w, a.get(1).map(String::as_str)).map(|_| None);
                        }
                        "feed" => {
                            super::kill_feed::signal_feed::stage(w);
                            if let Some(path) = a.get(1) {
                                super::ui_parity::apply_fixture(w, name, path)?;
                            }
                            return Ok(None);
                        }
                        _ => {}
                    }
                    w.resource_mut::<UiScene>().want = Some(name.to_string());
                    Ok(None)
                },
            )
            .add_systems(Update, stage);
    }
}

#[allow(clippy::too_many_arguments)]
fn stage(
    mut scene: ResMut<UiScene>,
    roots: Query<Entity, With<UiSceneRoot>>,
    theme: Res<Theme>,
    kit: Option<Res<KitFonts>>,
    units: Res<UiUnits>,
    mut images: ResMut<Assets<Image>>,
    mut icon_images: ResMut<IconImages>,
    mut slants: ResMut<Assets<SlantMaterial>>,
    mut ticks: ResMut<Assets<TickBarMaterial>>,
    mut passes: ResMut<Assets<PassMaterial>>,
    mut lines: ResMut<Assets<LinesMaterial>>,
    mut ambients: ResMut<Assets<chimera_ui::AmbientMaterial>>,
    mut commands: Commands,
) {
    // Again when the window's size or scale changes (and on the frame
    // the window is first measured).
    if scene.want == scene.shown && !(units.is_changed() && scene.shown.is_some()) {
        return;
    }
    for r in &roots {
        commands.entity(r).despawn();
    }
    scene.shown = scene.want.clone();
    if scene.want.as_deref() == Some("proof") {
        let mut m = Makers {
            theme: &theme,
            kit: kit.as_deref(),
            units: *units,
            images: &mut images,
            icon_images: &mut icon_images,
            slants: &mut slants,
            ticks: &mut ticks,
        };
        proof(&mut m, &mut passes, &mut lines, &mut ambients, &mut commands);
    }
}

struct Makers<'a> {
    theme: &'a Theme,
    kit: Option<&'a KitFonts>,
    units: UiUnits,
    images: &'a mut Assets<Image>,
    icon_images: &'a mut IconImages,
    slants: &'a mut Assets<SlantMaterial>,
    ticks: &'a mut Assets<TickBarMaterial>,
}

impl Makers<'_> {
    fn text(&self, role: Role, size_cq: f32, text: &str, tone: Tone) -> impl Bundle + use<> {
        let s = fonts::text(self.theme, self.kit, role, self.units.text_px(size_cq));
        (Text::new(s.case(text)), s.bundle(), TextColor(self.theme.tone(tone)))
    }

    fn icon(&mut self, key: &str, h_cq: f32, tone: Tone) -> impl Bundle + use<> {
        let px = self.units.phys(self.units.cq(h_cq)).round().max(1.0) as u32;
        let aspect = self.icon_images.icons().get(key).map_or(1.0, |i| i.aspect());
        let image = self.icon_images.get(self.images, key, px).unwrap_or_default();
        let (h, w) = (px as f32 / self.units.scale_factor, (aspect * px as f32).ceil() / self.units.scale_factor);
        (
            ImageNode { image, color: self.theme.tone(tone), ..default() },
            Node { width: px_val(w), height: px_val(h), ..default() },
        )
    }

    fn slant(&mut self, mat: SlantMaterial, w_cq: f32, h_cq: f32) -> impl Bundle + use<> {
        (
            MaterialNode(self.slants.add(mat)),
            Node { width: self.units.val(w_cq), height: self.units.val(h_cq), ..default() },
        )
    }

    fn tick_bar(&mut self, n: u32, w_cq: f32, h_cq: f32, lit: u32, hot: u32, skew: bool) -> impl Bundle + use<> {
        let u = self.units;
        let layout = TickLayout::new(u.phys(u.cq(w_cq)) as u32, u.phys(u.cq(h_cq)) as u32, n);
        let mut mat = TickBarMaterial::new(self.theme, n, layout);
        if skew {
            mat = mat.skewed(self.theme);
        }
        mat.set(lit, hot);
        (
            MaterialNode(self.ticks.add(mat)),
            Node {
                width: px_val((layout.w as f32 + layout.h as f32 * self.theme.tan()).ceil() / u.scale_factor),
                height: px_val(u.snap(u.cq(h_cq))),
                ..default()
            },
        )
    }
}

fn px_val(v: f32) -> Val {
    Val::Px(v)
}

fn row(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(gap), ..default() }
}

fn column(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(gap), ..default() }
}

/// The kit on one screen (module doc).
fn proof(m: &mut Makers, passes: &mut Assets<PassMaterial>, lines: &mut Assets<LinesMaterial>, ambients: &mut Assets<chimera_ui::AmbientMaterial>, commands: &mut Commands) {
    let u = m.units;
    let t = m.theme;
    let gap = u.cq(1.2);
    let root = commands
        .spawn((
            UiSceneRoot,
            Name::new("ui_scene proof"),
            GlobalZIndex(UI_SCENE_Z),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                padding: UiRect::axes(u.val(4.6), u.val(3.4)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(gap),
                ..default()
            },
            BackgroundColor(t.tone(Tone::Ink)),
        ))
        .id();
    // The ambient and the match end's lines behind, frozen (screenshots
    // compare alike).
    let amb = commands
        .spawn((
            MaterialNode(ambients.add(chimera_ui::AmbientMaterial::new(t).freeze(Some(6.0)))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(amb);
    let bg = commands
        .spawn((
            MaterialNode(lines.add(LinesMaterial::new(t).freeze(Some(6.0)))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .id();
    commands.entity(root).add_child(bg);

    // The lockup: the mark and the wordmark.
    let mark = m.icon("mark", 2.05 * 3.3, Tone::Fg);
    let lockup = commands.spawn(row(u.cq(1.8))).id();
    let mark_e = commands.spawn(mark).id();
    let words = commands.spawn(column(u.cq(1.0))).id();
    let ult = commands.spawn(m.text(Role::DisplayRegular, 3.3 * 0.62, "Ultimate", Tone::Fg2)).id();
    let gg = commands.spawn(m.text(Role::Wordmark, 3.3, "Gun Game", Tone::Fg)).id();
    commands.entity(words).add_children(&[ult, gg]);
    let tag = commands.spawn(m.text(Role::Mono, 0.62, "UI kit proof - Signal - M0", Tone::AccText)).id();
    commands.entity(lockup).add_children(&[mark_e, words, tag]);
    for (kind, label) in [(TagKind::You, "You"), (TagKind::Host, "Host"), (TagKind::Accent, "Rung 7")] {
        let e = widgets::spawn_pair(commands, widgets::tag(t, m.kit, &u, kind, label, 0.62));
        commands.entity(lockup).add_child(e);
    }
    let loss = commands.spawn(m.icon("loss", 0.72, Tone::Threat)).id();
    commands.entity(lockup).add_child(loss);
    let fused = widgets::spawn_you_host(commands, m.slants, t, m.kit, &u, 0.62);
    let bot = commands.spawn(widgets::bot_slot(m.images, m.icon_images, t, &u, true, 1.1, Tone::Fg2)).id();
    let k = widgets::spawn_pair(commands, widgets::kbd(t, m.kit, &u, "Tab", 0.62));
    commands.entity(lockup).add_children(&[fused, bot, k]);
    for ms in [12, 72, 140, 240] {
        let e = widgets::spawn_ping_meter(commands, m.slants, t, &u, 0.72 * 0.92, PingLevel::from_ms(t, ms));
        commands.entity(lockup).add_child(e);
    }
    commands.entity(root).add_child(lockup);

    let body = commands.spawn(row(u.cq(4.0))).id();
    commands.entity(body).insert(Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(u.cq(4.0)),
        align_items: AlignItems::FlexStart,
        ..default()
    });
    commands.entity(root).add_child(body);

    // Left: the type ramp.
    let ramp = commands.spawn(column(u.cq(0.7))).id();
    let lines_of = [
        (Role::DisplayBlack, 3.0, "Rank 07 / 14", Tone::Fg),
        (Role::DisplayBold, 1.36, "Play  Join  Options  Quit", Tone::Fg),
        (Role::DisplayBold, 0.94, "AK-47 - Combat Arms", Tone::Fg),
        (Role::DisplayRegular, 0.94, "Desert Eagle .357 Magnum", Tone::Fg2),
        (Role::Mono, 0.66, "K 12  D 04  +2 over lowgrav_larry", Tone::Fg2),
        (Role::Mono, 0.56, "Impact in 3 - stay down", Tone::Threat),
        (Role::Mono, 0.48, "Rifle - Counter-Strike: Source  0123456789", Tone::Fg3),
    ];
    for (role, size, text, tone) in lines_of {
        let label = format!("{} {:.2}cq", role.key(), size);
        let r = commands.spawn(row(u.cq(1.0))).id();
        let a = commands.spawn(m.text(Role::Mono, 0.42, &label, Tone::Fg3)).id();
        let b = commands.spawn(m.text(role, size, text, tone)).id();
        commands.entity(r).add_children(&[a, b]);
        commands.entity(ramp).add_child(r);
    }
    for (you, name) in [(true, "kestrel"), (false, "lowgrav_larry")] {
        let r = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(u.cq(0.6)),
                    width: u.val(24.0),
                    height: u.val(2.15),
                    padding: UiRect::horizontal(u.val(0.6)),
                    ..default()
                },
            ))
            .id();
        if you {
            commands.entity(r).insert((widgets::row_you(t, &u), bevy::picking::hover::Hovered::default()));
        }
        let slot = commands.spawn(widgets::bot_slot(m.images, m.icon_images, t, &u, !you, 0.9, Tone::Fg3)).id();
        let n = commands.spawn(m.text(Role::DisplayBold, 0.78, name, Tone::Fg)).id();
        // The second row as if hovered (its tick shown; no `Hovered` to hide it).
        let tick = commands.spawn(widgets::hover_tick(m.slants, t, &u, 2.15)).id();
        if !you {
            commands.entity(tick).insert(Visibility::Inherited);
        }
        commands.entity(r).add_children(&[slot, n, tick]);
        commands.entity(ramp).add_child(r);
    }
    commands.entity(body).add_child(ramp);

    // Right: bars, slants, the pass.
    let right = commands.spawn(column(u.cq(1.0))).id();
    let countdown = m.tick_bar(48, 20.0, 1.0, 30, 26, false);
    let chip = m.tick_bar(14, 9.24, 0.6, 9, 14, true);
    let c1 = commands.spawn(countdown).id();
    let c2 = commands.spawn(chip).id();
    let highlight = m.slant(
        SlantMaterial::new(t, t.tone(Tone::Fg5)).edges(false, true).accent(t.tone(Tone::Acc), u.phys(u.cq(0.32))),
        24.0,
        3.5,
    );
    let wipe_k = Ease::OUT.sample(0.6);
    let wiped = m.slant(SlantMaterial::new(t, t.tone(Tone::Acc)).clip(0.0, 1.0 - wipe_k), 21.0, 3.1);
    let banner = m.slant(SlantMaterial::new(t, t.tone(Tone::Threat)).ramp(0.15, 0.9), 24.0, 1.4);
    let h1 = commands.spawn(highlight).id();
    let h2 = commands.spawn(wiped).id();
    let h3 = commands.spawn(banner).id();
    let pips = commands.spawn(row(u.cq(0.3))).id();
    for i in 0..6 {
        let tone = if i < 4 { Tone::Acc } else { Tone::Fg4 };
        let p = m.slant(SlantMaterial::new(t, t.tone(tone)), 0.8, 0.5);
        let e = commands.spawn(p).id();
        commands.entity(pips).add_child(e);
    }
    let mark_px = (u.phys(u.cq(9.33 * 0.75))).round().max(1.0) as u32;
    let mark_img = m.icon_images.get(m.images, "mark", mark_px);
    let passes_row = commands.spawn(row(u.cq(1.0))).id();
    for p in [0.2, 0.42] {
        let mut pass = PassMaterial::new(t, mark_img.clone());
        pass.set_progress(p);
        let e = commands
            .spawn((
                MaterialNode(passes.add(pass)),
                Node { width: u.val(16.0 * 1.5), height: u.val(9.0 * 1.5), ..default() },
                Outline::new(Val::Px(u.hairline()), Val::ZERO, t.tone(Tone::Fg4)),
            ))
            .id();
        commands.entity(passes_row).add_child(e);
    }
    let pass_e = passes_row;
    commands.entity(right).add_children(&[c1, c2, h1, h2, h3, pips, pass_e]);
    commands.entity(body).add_child(right);

    // The gun icons at the kill feed's size and twice it, then the marks.
    for (h, tone) in [(1.25, Tone::Fg), (2.5, Tone::Fg2)] {
        let r = commands.spawn(row(u.cq(1.2))).id();
        for key in icons::GUNS {
            let e = commands.spawn(m.icon(key, h, tone)).id();
            commands.entity(r).add_child(e);
        }
        commands.entity(root).add_child(r);
    }
    let r = commands.spawn(row(u.cq(1.2))).id();
    for (key, tone) in [("crown", Tone::Acc), ("trophy", Tone::Acc), ("kill", Tone::Fg), ("death", Tone::Fg2), ("warn", Tone::Threat)] {
        let e = commands.spawn(m.icon(key, 1.25, tone)).id();
        commands.entity(r).add_child(e);
    }
    commands.entity(root).add_child(r);
}
