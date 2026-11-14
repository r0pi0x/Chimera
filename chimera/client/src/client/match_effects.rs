//! The match's temporary effects on screen (docs/plans/active/ugg-ui.md,
//! M3): what `chimera_sim::minigame::effects::MatchEffects` lists (a
//! host's surprises: an air strike, moon gravity; replicated with the
//! match), drawn by the shell for any app. Each effect is placed on the
//! UI clock when first seen (`EffectsView`: its start on the simulation's
//! clock mapped to the UI's), so its moves are smooth and its countdown
//! matches the simulation's.
//!
//! **Signal** (`signal::Look::on`; numbers from `motion.dur.airstrike`
//! and `layout.airstrike`, the look book's by default):
//!
//! - **The banner** (the newest effect's, 15.5 cq down, above the
//!   crosshair, 3.3 cq tall): a thin band tinted in the threat colour (a
//!   threat; else the accent) fading out at both ends, hairlines over and
//!   under, shadowed type, so the map shows through and around it. It
//!   opens from the centre at 0.25 s (0.3 s), its title's letter spacing
//!   closing from 0.3 to 0.08 em; a threat's carries the warning mark and
//!   counts down to its impact. At 2 s it folds to the centre (`expo_in`,
//!   0.28 s) into its chip.
//! - **The edge** (a threat only): the screen's edges glow red
//!   (`chimera_ui::EdgeMaterial`, its middle clear: never a full-screen
//!   fill), a pulse on each second of the countdown (`Ease::Pulse`: 80 ms
//!   attack, 320 ms decay) over a base of 0.2, 1.3 times at the impact,
//!   then dying away. No flash.
//! - **The chips** (top left, 2.2 cq in, 13.4 cq wide, one slot each
//!   3.05 cq down: the place for every temporary effect): an ink plate
//!   with the effect's name and a line under it; a threat with the
//!   warning mark and a threat bar, "Impact in N" then "Impact • stay
//!   down"; a timed effect with a ring counting down its seconds
//!   (`chimera_ui::RingMaterial`) and its detail. A chip wipes in from the
//!   left as its banner folds (0.3 s), leaves 0.2 s before its end;
//!   threats sit first, the others by their start, and a chip moving slot
//!   slides (`cubic_in_out`, 0.24 s).
//!
//! **Plain** (any other theme: Lucker Party, the plain apps): the warning
//! as a text banner a third of the way down in the effect's colour
//! ("AIR STRIKE INCOMING!", "TAKE COVER!"; "Moon gravity!", "for 60
//! seconds"), a short flash of that colour as it starts, the countdown
//! line in the last 5 s and the line at its natural end, as the host's HUD
//! messages were.
//!
//! `ui_scene airstrike [t]` stages the look book's loop (moon gravity on,
//! an air strike coming: banner, pulses, the fold, the stack shuffling)
//! on the UI clock, frozen at `t` s into it with a time.
//!
//! Known shortcuts: the banner's type has a hard drop shadow, no glow
//! (Bevy has no text blur); the
//! banner's tint is a gradient in linear light (a touch brighter than the
//! look book's); the plain look's banner is the overlay's face, not the
//! map HUD's.

use bevy::prelude::*;
use chimera_core::core::SimClock;
use chimera_sim::minigame::effects::{Effect, MatchEffects};
use chimera_ui::{EdgeMaterial, Ease, RingMaterial, Role, Tone, UiClock};

use super::signal::{self, Look, UiOpacity};

/// The effects' screens.
pub struct MatchEffectsPlugin;

impl Plugin for MatchEffectsPlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        signal::setup(app);
        app.init_resource::<EffectsView>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (sync, (edge, banner, chips, plain)).chain());
    }
}

/// An effect as shown: placed on the UI clock (`t0`: its start there).
#[derive(Clone, Debug, PartialEq)]
pub struct ShownEffect {
    pub e: Effect,
    pub t0: f64,
}

impl ShownEffect {
    /// Seconds since it started, at UI time `now`.
    pub fn t(&self, now: f64) -> f32 {
        (now - self.t0) as f32
    }

    /// Its impact and end, seconds after its start.
    pub fn impact(&self) -> Option<f32> {
        self.e.impact_at.map(|i| (i - self.e.starts) as f32)
    }

    pub fn end(&self) -> f32 {
        (self.e.ends - self.e.starts) as f32
    }
}

/// What the screens show now (`sync`).
#[derive(Resource, Clone, Debug, Default)]
pub struct EffectsView {
    pub shown: Vec<ShownEffect>,
}

/// A staged loop (`ui_scene airstrike`): `effects` (times in seconds of
/// the loop) again every `period` s from `start` on the UI clock.
#[derive(Resource, Clone, Debug)]
pub struct EffectsScript {
    pub start: f64,
    pub period: f32,
    pub effects: Vec<Effect>,
}

/// The look book's loop: moon gravity on (7.4 s of its 30 left), an air
/// strike warned at 0 striking at 4.2 s, over at 6 s; every 8 s.
pub fn airstrike_script(start: f64) -> EffectsScript {
    let strike = Effect {
        id: 2,
        kind: "air_strike".into(),
        title: "Air strike".into(),
        detail: "Take cover".into(),
        threat: true,
        color: [255, 48, 32],
        starts: 0.0,
        impact_at: Some(4.2),
        ends: 6.0,
        ..default()
    };
    let moon = Effect {
        id: 1,
        kind: "moon_gravity".into(),
        title: "Moon gravity".into(),
        detail: "Jump higher • fall slower".into(),
        color: [50, 110, 255],
        starts: -22.6,
        ends: 7.4,
        ..default()
    };
    EffectsScript { start, period: 8.0, effects: vec![moon, strike] }
}

/// Stages the loop from `start` (UI clock), or clears it.
pub fn stage(world: &mut World, start: Option<f64>) {
    match start {
        Some(s) => {
            world.insert_resource(airstrike_script(s));
        }
        None => {
            world.remove_resource::<EffectsScript>();
        }
    }
}

/// How long a gone effect stays shown after its end (s): its chip's exit
/// and the plain look's line at its end.
const KEEP: f32 = 3.0;

/// The view from the effects (or the staged loop).
fn sync(
    clock: Res<UiClock>,
    sim: Option<Res<SimClock>>,
    effects: Option<Res<MatchEffects>>,
    script: Option<Res<EffectsScript>>,
    mut view: ResMut<EffectsView>,
) {
    let now = clock.now;
    if let Some(s) = script {
        let cycle = ((now - s.start) / s.period as f64).floor().max(0.0);
        let base = s.start + cycle * s.period as f64;
        let shown: Vec<ShownEffect> =
            s.effects.iter().map(|e| ShownEffect { e: Effect { id: e.id + 2 * cycle as u32, ..e.clone() }, t0: base + e.starts }).collect();
        if view.shown != shown {
            view.shown = shown;
        }
        return;
    }
    let sim_now = sim.map_or(0.0, |c| c.now);
    let list = effects.as_ref().map_or(&[][..], |m| &m.effects[..]);
    let mut next: Vec<ShownEffect> = Vec::with_capacity(list.len());
    for e in list {
        let t0 = view
            .shown
            .iter()
            .find(|s| s.e.id == e.id)
            .map_or_else(|| now - (sim_now - e.starts), |s| s.t0);
        next.push(ShownEffect { e: e.clone(), t0 });
    }
    // Gone from the list: kept a while after its end (on the UI clock).
    for s in &view.shown {
        if !next.iter().any(|n| n.e.id == s.e.id) && s.t(now) < s.end() + KEEP {
            next.push(s.clone());
        }
    }
    if view.shown != next {
        view.shown = next;
    }
}

fn c01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn e_out(p: f32) -> f32 {
    Ease::ExpoOut.sample(c01(p))
}

fn e_in(p: f32) -> f32 {
    Ease::ExpoIn.sample(c01(p))
}

fn e_io(p: f32) -> f32 {
    Ease::CubicInOut.sample(c01(p))
}

/// The air strike's numbers (`motion.dur.airstrike`, `layout.airstrike`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrikeTimes {
    pub banner_at: f32,
    pub banner: f32,
    pub fold_at: f32,
    pub fold: f32,
    pub stack: f32,
    pub banner_top: f32,
    pub banner_h: f32,
    pub chip_w: f32,
    pub pitch: f32,
}

impl StrikeTimes {
    pub fn of(look: &Look) -> StrikeTimes {
        StrikeTimes {
            banner_at: look.dur("airstrike", "banner_at", 0.25),
            banner: look.dur("airstrike", "banner", 0.3),
            fold_at: look.dur("airstrike", "fold_at", 2.0),
            fold: look.dur("airstrike", "fold", 0.28),
            stack: look.dur("airstrike", "chips_stack", 0.24),
            banner_top: look.layout("airstrike", "banner_top", 15.5),
            banner_h: look.layout("airstrike", "banner_h", 3.3),
            chip_w: look.layout("airstrike", "chip_w", 13.4),
            pitch: look.layout("airstrike", "chip_pitch", 3.05),
        }
    }

    /// The banner's opening at `t` (0-1: closed to open; folded after).
    pub fn banner_open(&self, t: f32) -> f32 {
        e_out((t - self.banner_at) / self.banner) * (1.0 - e_in((t - self.fold_at) / self.fold))
    }

    /// When the chip starts to wipe in.
    pub fn chip_at(&self) -> f32 {
        self.fold_at + 0.12
    }

    /// The chip's wipe (0-1) at `t` for an effect ending at `end`.
    pub fn chip_in(&self, t: f32, end: f32) -> f32 {
        e_out((t - self.chip_at()) / 0.3) * (1.0 - e_in((t - (end - 0.2)) / 0.2))
    }
}

/// The edge's opacity at `t` for a threat striking at `impact` and over
/// at `end`: a pulse on each second of the countdown and 1.3 times at
/// the impact over a base of 0.2 while it comes, dying away after.
pub fn edge_opacity(t: f32, impact: f32, end: f32) -> f32 {
    if t < 0.0 || t > end + 2.0 {
        return 0.0;
    }
    let pulse = |x: f32| Ease::Pulse.sample(x);
    let mut i: f32 = 0.0;
    let mut k = 1.0;
    while impact - k >= -1e-3 {
        i = i.max(pulse(t - (impact - k)));
        k += 1.0;
    }
    i = i.max(1.3 * pulse(t - impact));
    let base = if t > 0.2 && t < impact {
        0.2
    } else if t >= impact {
        0.25 * (-(t - impact) / 0.5).exp()
    } else {
        0.0
    };
    (base + 0.68 * i).min(1.0)
}

/// The edge glow and its opacity now (tests read it).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgePulse(pub f32);

/// The banner's root, its clip (its width the opening), its body (the
/// effect it shows), the title (its letter spacing) and the count.
#[derive(Component)]
pub struct BannerRoot;
/// The clip, and how far open (0-1) the banner is (tests read it).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BannerClip(pub f32);
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerBody(pub u32);
#[derive(Component)]
pub struct BannerTitle;
#[derive(Component)]
pub struct BannerCount;

/// The chips' root (top left) and a chip: its effect and its slot's move.
#[derive(Component)]
pub struct ChipsRoot;
#[derive(Component, Clone, Debug, PartialEq)]
pub struct EffectChip {
    pub id: u32,
    pub slot: usize,
    /// The slot's move: from y (cq), since (UI time).
    pub from_y: f32,
    pub since: f64,
}
/// A chip's line: before the number, the number, after it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipText {
    Pre,
    Num,
    Post,
}
/// A chip's ring and its count.
#[derive(Component)]
pub struct ChipRing;

/// The plain look's banner and flash.
#[derive(Component)]
pub struct PlainText;
#[derive(Component)]
pub struct PlainFlash;

const EDGE_Z: i32 = 50;
const BANNER_Z: i32 = 70;
const CHIPS_Z: i32 = 75;

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    let full = Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() };
    commands.spawn((EdgePulse(0.0), ChildOf(layer.0), Visibility::Hidden, GlobalZIndex(EDGE_Z), Pickable::IGNORE, full.clone()));
    commands
        .spawn((
            BannerRoot,
            ChildOf(layer.0),
            Visibility::Hidden,
            GlobalZIndex(BANNER_Z),
            Pickable::IGNORE,
            Node { position_type: PositionType::Absolute, left: px(0.0), width: percent(100.0), justify_content: JustifyContent::Center, ..default() },
        ))
        .with_children(|r| {
            r.spawn((
                BannerClip(0.0),
                Pickable::IGNORE,
                Node { overflow: Overflow::clip(), justify_content: JustifyContent::Center, flex_shrink: 0.0, width: px(0.0), ..default() },
            ));
        });
    commands.spawn((
        ChipsRoot,
        ChildOf(layer.0),
        GlobalZIndex(CHIPS_Z),
        Pickable::IGNORE,
        Node { position_type: PositionType::Absolute, ..default() },
    ));
    commands.spawn((PlainFlash, ChildOf(layer.0), Visibility::Hidden, GlobalZIndex(EDGE_Z), Pickable::IGNORE, BackgroundColor(Color::NONE), full));
    commands
        .spawn((
            ChildOf(layer.0),
            GlobalZIndex(BANNER_Z),
            Pickable::IGNORE,
            Node { position_type: PositionType::Absolute, left: px(0.0), width: percent(100.0), top: percent(30.0), justify_content: JustifyContent::Center, ..default() },
        ))
        .with_children(|r| {
            r.spawn((
                PlainText,
                Visibility::Hidden,
                Text::new(""),
                TextColor(Color::WHITE),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                TextShadow::default(),
                Pickable::IGNORE,
            ));
        });
}

/// The edge glow: the strongest threat's.
fn edge(
    look: Look,
    clock: Res<UiClock>,
    view: Res<EffectsView>,
    mut mats: Option<ResMut<Assets<EdgeMaterial>>>,
    node: Single<(Entity, &mut EdgePulse, &mut Visibility, Option<&MaterialNode<EdgeMaterial>>)>,
    mut size: Local<(u32, u32)>,
    mut commands: Commands,
) {
    let (e, mut pulse, mut vis, mat) = node.into_inner();
    let o = if look.on() {
        view.shown
            .iter()
            .filter(|s| s.e.threat)
            .map(|s| edge_opacity(s.t(clock.now), s.impact().unwrap_or(s.end()), s.end()))
            .fold(0.0, f32::max)
    } else {
        0.0
    };
    if pulse.0 != o {
        pulse.0 = o;
    }
    vis.set_if_neq(if o > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
    let Some(mats) = mats.as_deref_mut() else { return };
    let key = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    let phys = |cq: f32| look.units.phys(look.units.cq(cq));
    match mat {
        Some(m) if *size == key => {
            if let Some(m) = mats.get(&m.0)
                && m.params.x == o
            {
                return;
            }
            if let Some(mut m) = mats.get_mut(&m.0) {
                m.set_opacity(o);
            }
        }
        _ => {
            *size = key;
            let mut m = EdgeMaterial::new(look.tone(Tone::Threat), phys(7.0), phys(1.2), phys(0.22).max(1.0));
            m.set_opacity(o);
            commands.entity(e).insert(MaterialNode(mats.add(m)));
        }
    }
}

/// The newest effect's banner while it shows.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn banner(
    look: Look,
    clock: Res<UiClock>,
    view: Res<EffectsView>,
    mut root: Single<(&mut Visibility, &mut Node), With<BannerRoot>>,
    clip: Single<(Entity, &mut Node, &mut BannerClip), Without<BannerRoot>>,
    body: Query<(Entity, &BannerBody, &ComputedNode)>,
    mut title: Query<(&mut bevy::text::LetterSpacing, &mut Text), (With<BannerTitle>, Without<BannerCount>)>,
    mut count: Query<&mut Text, With<BannerCount>>,
    mut commands: Commands,
) {
    let d = StrikeTimes::of(&look);
    let now = clock.now;
    let shown = view
        .shown
        .iter()
        .filter(|s| look.on() && s.t(now) >= 0.0 && s.t(now) < d.fold_at + d.fold && s.t(now) < s.end())
        .max_by(|a, b| a.t0.total_cmp(&b.t0));
    let (vis, rnode) = &mut *root;
    let (clip_e, mut clip_node, mut clip_open) = clip.into_inner();
    let Some(s) = shown else {
        clip_open.set_if_neq(BannerClip(0.0));
        vis.set_if_neq(Visibility::Hidden);
        for (e, _, _) in &body {
            commands.entity(e).despawn();
        }
        return;
    };
    vis.set_if_neq(Visibility::Inherited);
    let top = look.val(d.banner_top);
    if rnode.top != top {
        rnode.top = top;
    }
    let t = s.t(now);
    // The body for this effect (rebuilt for another).
    let mut width = None;
    let resized = look.units.is_changed() || look.theme.is_changed();
    for (e, b, c) in &body {
        if b.0 == s.e.id && !resized {
            width = Some(c.size.x * c.inverse_scale_factor);
        } else {
            commands.entity(e).despawn();
        }
    }
    if width.is_none() {
        build_banner(&look, &d, s, clip_e, &mut commands);
    }
    let open = d.banner_open(t);
    clip_open.set_if_neq(BannerClip(open));
    let w = px((open * width.unwrap_or(0.0)).round());
    if clip_node.width != w {
        clip_node.width = w;
        clip_node.height = look.val(d.banner_h);
    }
    let em = look.units.text_px(1.3);
    for (mut sp, mut text) in &mut title {
        let v = bevy::text::LetterSpacing::Px((em * (0.3 + (0.08 - 0.3) * e_out((t - 0.3) / 0.45))).round());
        if *sp != v {
            *sp = v;
            // Laid out again with it.
            text.set_changed();
        }
    }
    if let Some(i) = s.impact() {
        let n = ((i - t).ceil() as i32).max(1).to_string();
        for mut c in &mut count {
            if c.0 != n {
                c.0 = n.clone();
            }
        }
    }
}

fn build_banner(look: &Look, d: &StrikeTimes, s: &ShownEffect, clip: Entity, commands: &mut Commands) {
    let tint = if s.e.threat { Tone::Threat } else { Tone::Acc };
    let c = look.theme.tone(tint);
    let band = |a: f32| {
        bevy::ui::BackgroundGradient(vec![bevy::ui::Gradient::Linear(bevy::ui::LinearGradient::to_right(vec![
            bevy::ui::ColorStop::new(c.with_alpha(0.0), percent(0.0)),
            bevy::ui::ColorStop::new(c.with_alpha(a), percent(18.0)),
            bevy::ui::ColorStop::new(c.with_alpha(a), percent(82.0)),
            bevy::ui::ColorStop::new(c.with_alpha(0.0), percent(100.0)),
        ]))])
    };
    let line = |a: f32| {
        bevy::ui::BackgroundGradient(vec![bevy::ui::Gradient::Linear(bevy::ui::LinearGradient::to_right(vec![
            bevy::ui::ColorStop::new(c.with_alpha(0.0), percent(0.0)),
            bevy::ui::ColorStop::new(c.with_alpha(a), percent(50.0)),
            bevy::ui::ColorStop::new(c.with_alpha(0.0), percent(100.0)),
        ]))])
    };
    let hair = look.hair();
    // Shadowed type (no blur in Bevy: a dark copy a little down), so it
    // reads over a bright sky.
    let shadow = TextShadow { offset: Vec2::new(0.0, look.px(0.12).max(1.0)), color: Color::srgba(0.0, 0.0, 0.0, 0.85) };
    commands.entity(clip).with_children(|cl| {
        cl.spawn((
            BannerBody(s.e.id),
            Node {
                height: look.val(d.banner_h),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: look.val(1.1),
                padding: UiRect::horizontal(look.val(1.8)),
                ..default()
            },
            band(0.16),
            Pickable::IGNORE,
        ))
        .with_children(|b| {
            for top in [true, false] {
                let mut n = Node { position_type: PositionType::Absolute, left: percent(8.0), right: percent(8.0), height: px(hair), ..default() };
                if top {
                    n.top = px(0.0);
                } else {
                    n.bottom = px(0.0);
                }
                b.spawn((n, line(0.9), Pickable::IGNORE));
            }
            if s.e.threat {
                signal::icons::spawn(b, look, "warn", look.px(1.2), look.tone(Tone::Threat));
            }
            let title = if s.e.threat { format!("{} incoming", s.e.title) } else { s.e.title.clone() };
            b.spawn((look.text(Role::DisplayBold, 1.3, look.tone(Tone::Fg), &title.to_uppercase()), shadow, BannerTitle, Pickable::IGNORE));
            let sub = if s.e.impact_at.is_some() { format!("{} • impact in", s.e.detail) } else { s.e.detail.clone() };
            b.spawn((look.text(Role::Mono, 0.56, look.tone(Tone::Fg), &sub.to_uppercase()), shadow, Pickable::IGNORE));
            if s.e.impact_at.is_some() {
                b.spawn((
                    Node { min_width: look.val(1.0), ..default() },
                    look.text(Role::DisplayBold, 1.3, look.tone(Tone::Threat), ""),
                    BannerCount,
                    shadow,
                    Pickable::IGNORE,
                ));
            }
        });
    });
}

/// The chips: one an effect past its fold, stacked, sliding between
/// slots.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn chips(
    look: Look,
    clock: Res<UiClock>,
    view: Res<EffectsView>,
    mut rings: Option<ResMut<Assets<RingMaterial>>>,
    mut root: Single<&mut Node, With<ChipsRoot>>,
    root_e: Single<Entity, With<ChipsRoot>>,
    mut chips: Query<(Entity, &mut EffectChip, &mut Node, &mut UiTransform, &mut UiOpacity), Without<ChipsRoot>>,
    mut texts: Query<(&ChildOf, &ChipText, &mut Text)>,
    ring_nodes: Query<(&ChildOf, &MaterialNode<RingMaterial>), With<ChipRing>>,
    mut ring_texts: Query<(&ChildOf, &mut Text), (With<ChipRing>, Without<ChipText>)>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    let d = StrikeTimes::of(&look);
    let now = clock.now;
    let (left, top) = (look.val(2.2), look.val(2.2));
    if root.left != left || root.top != top {
        root.left = left;
        root.top = top;
    }
    // Which effects have a chip now, threats first, then by start.
    let mut on: Vec<&ShownEffect> = if look.on() {
        view.shown.iter().filter(|s| d.chip_in(s.t(now), s.end()) > 0.0 || (s.t(now) >= d.chip_at() && s.t(now) < s.end())).collect()
    } else {
        Vec::new()
    };
    on.sort_by(|a, b| b.e.threat.cmp(&a.e.threat).then(a.t0.total_cmp(&b.t0)));
    // Gone (or laid out for another window size): despawned.
    let resized = look.units.is_changed() || look.theme.is_changed();
    for (e, chip, ..) in &chips {
        if resized || !on.iter().any(|s| s.e.id == chip.id) {
            commands.entity(e).despawn();
        }
    }
    let have: Vec<u32> = if resized { Vec::new() } else { chips.iter().map(|(_, c, ..)| c.id).collect() };
    for (slot, s) in on.iter().enumerate() {
        if !have.contains(&s.e.id) {
            spawn_chip(&look, &d, s, slot, now, *root_e, rings.as_deref_mut(), &mut commands);
        }
    }
    for (e, mut chip, mut node, mut tf, mut op) in &mut chips {
        let Some((slot, s)) = on.iter().enumerate().find(|(_, s)| s.e.id == chip.id) else { continue };
        let y_now = |c: &EffectChip| {
            let p = e_io((now - c.since) as f32 / d.stack);
            c.from_y + (c.slot as f32 * d.pitch - c.from_y) * p
        };
        if chip.slot != slot {
            let y = y_now(&chip);
            chip.from_y = y;
            chip.slot = slot;
            chip.since = now;
        }
        let t = s.t(now);
        let p = d.chip_in(t, s.end()).max(if t >= d.chip_at() + 0.3 && t < s.end() - 0.2 { 1.0 } else { 0.0 });
        let w = px((p * look.px(d.chip_w)).round());
        if node.width != w {
            node.width = w;
        }
        let next = Val2::new(px(look.units.cq(-(1.0 - p) * 1.2)), px(look.units.cq(y_now(&chip))));
        if tf.translation != next {
            tf.translation = next;
        }
        op.set_if_neq(UiOpacity(if p > 0.0 { 1.0 } else { 0.0 }));
        // Its line and ring.
        let (pre, num, post) = match s.impact() {
            Some(i) if t < i => ("Impact in".to_string(), ((i - t).ceil() as i32).max(1).to_string(), String::new()),
            Some(_) => (String::new(), "Impact".to_string(), "• stay down".to_string()),
            None => (s.e.detail.clone(), String::new(), String::new()),
        };
        for (parent, which, mut text) in &mut texts {
            if !is_under(parent.parent(), e, &parents) {
                continue;
            }
            let want = match which {
                ChipText::Pre => &pre,
                ChipText::Num => &num,
                ChipText::Post => &post,
            }
            .to_uppercase();
            if text.0 != want {
                text.0 = want;
            }
        }
        let left = (s.end() - t).max(0.0);
        let total = s.end().max(1e-3);
        for (parent, m) in &ring_nodes {
            if is_under(parent.parent(), e, &parents)
                && let Some(rings) = rings.as_deref_mut()
                && let Some(mat) = rings.get(&m.0)
                && (mat.params.x - left / total).abs() > 1e-4
                && let Some(mut mat) = rings.get_mut(&m.0)
            {
                mat.set_share(left / total);
            }
        }
        let n = (left.ceil() as i32).to_string();
        for (parent, mut text) in &mut ring_texts {
            if is_under(parent.parent(), e, &parents) && text.0 != n {
                text.0 = n.clone();
            }
        }
    }
}

/// `e` is `root` or under it.
fn is_under(mut e: Entity, root: Entity, parents: &Query<&ChildOf>) -> bool {
    loop {
        if e == root {
            return true;
        }
        match parents.get(e) {
            Ok(p) => e = p.parent(),
            Err(_) => return false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_chip(look: &Look, d: &StrikeTimes, s: &ShownEffect, slot: usize, now: f64, root: Entity, rings: Option<&mut Assets<RingMaterial>>, commands: &mut Commands) {
    let y = slot as f32 * d.pitch;
    let ring = (!s.e.threat).then(|| rings.map(|r| r.add(RingMaterial::new(look.tone(Tone::Fg4), look.tone(Tone::Fg), look.units.phys(look.units.cq(2.0)) * 2.4 / 36.0)))).flatten();
    commands.entity(root).with_children(|r| {
        r.spawn((
            EffectChip { id: s.e.id, slot, from_y: y, since: now },
            Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), overflow: Overflow::clip(), width: px(0.0), ..default() },
            UiTransform::default(),
            UiOpacity(0.0),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: look.val(d.chip_w),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    column_gap: look.val(0.7),
                    padding: UiRect::new(look.val(0.45), look.val(0.9), look.val(0.45), look.val(0.45)),
                    ..default()
                },
                BackgroundColor(look.tone(Tone::Plate)),
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                if s.e.threat {
                    b.spawn((
                        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), bottom: px(0.0), width: look.val(0.22), ..default() },
                        BackgroundColor(look.tone(Tone::Threat)),
                        Pickable::IGNORE,
                    ));
                }
                b.spawn((
                    Node { width: look.val(2.0), height: look.val(2.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                    Pickable::IGNORE,
                ))
                .with_children(|ic| {
                    if s.e.threat {
                        signal::icons::spawn(ic, look, "warn", look.px(1.2), look.tone(Tone::Threat));
                    } else {
                        let mut n = ic.spawn((
                            ChipRing,
                            Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
                            Pickable::IGNORE,
                        ));
                        if let Some(h) = ring.clone() {
                            n.insert(MaterialNode(h));
                        }
                        ic.spawn((ChipRing, look.text(Role::Mono, 0.58, look.tone(Tone::Fg), ""), Pickable::IGNORE));
                    }
                });
                b.spawn((Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.38), min_width: px(0.0), ..default() }, Pickable::IGNORE))
                    .with_children(|col| {
                        col.spawn((look.text(Role::DisplayBold, 0.7, look.tone(Tone::Fg), &s.e.title.to_uppercase()), Pickable::IGNORE));
                        col.spawn((Node { column_gap: look.val(0.25), ..default() }, Pickable::IGNORE)).with_children(|l| {
                            l.spawn((ChipText::Pre, look.text(Role::Mono, 0.5, look.tone(Tone::Fg2), ""), Pickable::IGNORE));
                            l.spawn((ChipText::Num, look.text(Role::Mono, 0.5, look.tone(Tone::Fg), ""), Pickable::IGNORE));
                            l.spawn((ChipText::Post, look.text(Role::Mono, 0.5, look.tone(Tone::Fg2), ""), Pickable::IGNORE));
                        });
                    });
            });
        });
    });
}

/// The plain look's line at `t` for an effect: (text, alpha) while one
/// shows (module doc).
pub fn plain_line(s: &ShownEffect, t: f32) -> Option<(String, f32)> {
    let fade = |since: f32, until: f32| c01((t - since) / 0.1).min(c01((until - t) / 0.5));
    let end = s.end();
    let hold = match s.impact() {
        Some(i) => i + 2.0,
        None => 3.0,
    };
    if t >= 0.0 && t < hold.min(end.max(hold)) {
        let text = if s.e.threat {
            format!("{} incoming!\n{}!", s.e.title, s.e.detail).to_uppercase()
        } else if end >= 1.5 {
            format!("{}!\nfor {:.0} seconds", s.e.title, end)
        } else {
            format!("{}!", s.e.title)
        };
        return Some((text, fade(0.0, hold)));
    }
    let left = (end - t).ceil();
    if !s.e.countdown.is_empty() && t >= 3.0 && t < end && left <= 5.0 {
        let since = end - left;
        return Some((s.e.countdown.replace("{n}", &format!("{left:.0}")), fade(since, since + 1.2)));
    }
    if !s.e.over.is_empty() && !s.e.stopped && t >= end && t < end + 2.5 {
        return Some((s.e.over.clone(), fade(end, end + 2.5)));
    }
    None
}

/// The plain look (module doc).
#[allow(clippy::type_complexity)]
fn plain(
    look: Look,
    clock: Res<UiClock>,
    view: Res<EffectsView>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    mut text: Single<(&mut Text, &mut TextColor, &mut TextFont, &mut Visibility), With<PlainText>>,
    mut flash: Single<(&mut BackgroundColor, &mut Visibility), (With<PlainFlash>, Without<PlainText>)>,
) {
    let now = clock.now;
    let line = (!look.on())
        .then(|| {
            view.shown
                .iter()
                .rev()
                .find_map(|s| plain_line(s, s.t(now)).map(|(t, a)| (t, a, s.e.color)))
        })
        .flatten();
    let (t, c, font, vis) = &mut *text;
    match line {
        Some((s, a, [r, g, b])) => {
            vis.set_if_neq(Visibility::Inherited);
            if t.0 != s {
                t.0 = s;
            }
            let col = Color::srgba_u8(r, g, b, (a * 255.0) as u8);
            if c.0 != col {
                c.0 = col;
            }
            // The size map logic's HUD text had: 24 px at 480 lines.
            let f = super::overlay::font(fonts.as_deref(), 24.0 * look.units.height / 480.0, true);
            if font.font_size != f.font_size {
                **font = f;
            }
        }
        None => {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    // A short flash of the effect's colour as it starts.
    let lit = (!look.on())
        .then(|| {
            view.shown.iter().find_map(|s| {
                let t = s.t(now);
                let (a, dur) = if s.e.threat { (90.0 / 255.0, 0.8) } else { (60.0 / 255.0, 0.6) };
                (t >= 0.0 && t < dur).then(|| (s.e.color, a * (1.0 - t / dur)))
            })
        })
        .flatten();
    let (bg, fvis) = &mut *flash;
    match lit {
        Some(([r, g, b], a)) => {
            fvis.set_if_neq(Visibility::Inherited);
            let col = super::widgets::display_alpha(Color::srgba_u8(r, g, b, 255).with_alpha(a));
            if bg.0 != col {
                bg.0 = col;
            }
        }
        None => {
            fvis.set_if_neq(Visibility::Hidden);
        }
    }
}
