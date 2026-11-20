//! The sandbox's HUD (the design rules, sections 18 to 21; revision 2's
//! `sb2-rest`, `sb2-chip`, `sb2-tab-a` and `sb2-pins`), under the Signal
//! theme while a sandbox plays (the race chip stands down then:
//! `race_chip::gather`).
//!
//! - **Top left**: the sandbox mark (an accent slant, SANDBOX) and two key
//!   hints, B Change gun and T Bench; with Tab held a white TAB cap and
//!   HELD beside the mark.
//! - **The gun chip** (top centre, 1.9 cq down, as wide as its name needs,
//!   12 cq at least): the gun's icon in a 3.4 x 1.9 cq box, its name
//!   (Display Bold 1.02 cq) and its game's full name under it (Mono .48
//!   cq). Under the chip the condition tags (`SandboxOptions::tags`: Take
//!   no damage only while Tab is held), or, while a picked gun downloads
//!   before it's in hand (`SandboxUi::pending`), the Next strip in their
//!   place: NEXT, its icon and name and the progress component (variant
//!   B, "64% · 3.1 of 4.8 MB"; queued a dim outline; failed: R retries).
//!   The mechanics' own waiting pick (`rung_command::GunPending`,
//!   `SandboxUi::pick_held`) shows there the same; with nothing to
//!   download in the units' list it reads "Loading". Under those, the
//!   mechanics' last word (`SandboxUi::said`: a switch refused, a
//!   waiting pick dropped), a threat mark before it.
//! - **Tab held** (option A, one column at the left from 5.6 cq down, 21
//!   cq wide): the last-kill card (LAST KILL, how long ago; the gun and
//!   the target, protection, distance and motion; hits, first hit to kill,
//!   distance in big Mono; the hits as slanted ticks with their damage and
//!   where; then the four kills before it, a line each), the options card
//!   (each option on or off; T changes them) and the keys card.
//! - **The pinned bar** (bottom centre, 7.4 cq up): six slots 6.4 x 3.6
//!   cq, each its key cap, its game's short name, the icon and the gun's
//!   name; the gun in hand white; an empty one EMPTY. Shown on a number
//!   key or Tab, fading out over `FADE` s after `model::PIN_IDLE` s idle
//!   (its alpha in steps, `ALPHA_STEPS`, so a fade rebuilds it ten times).
//!   Hidden while Change gun is open (that view draws its own).
//!
//! Rebuilt only when what it shows changes (`HudShown`), as the race chip
//! is. Look-book names (`UiParityId`): `sb.mark`, `sb.chip`, `sb.chip.name`,
//! `sb.chip.game`, `sb.conds`, `sb.next`, `sb.tab`, `sb.kill`, `sb.options`,
//! `sb.keys`, `sb.pins`, `sb.pin.<n>` (from 1).
//!
//! The keys card's K names the set-up it repeats ("Repeat: 2 targets, 1
//! prop", `layout::SandboxLayout`), greyed while there is none.
//!
//! Known shortcuts: the kill card's "12 s ago" is rounded to whole
//! seconds and only rebuilt each second; Q's quick panel isn't built (not
//! listed).

use bevy::prelude::*;
use chimera_ui::{Role, Tone, UiParityId, UiStyle, controls::Ui};

use super::{
    SandboxCatalogue, SandboxScreen, SandboxUi,
    link::{KillCard, KillLog, SandboxOption, SandboxOptions, TargetMotion},
    model::{PIN_SLOTS, SandboxGun},
    units::{UnitProgress, UnitState, size_words},
};
use crate::client::signal::{self, Look};

/// The pinned bar's fade, s.
pub const FADE: f64 = 0.5;
/// The bar's alpha steps while fading.
pub const ALPHA_STEPS: f32 = 10.0;

/// The HUD's root (tests find it).
#[derive(Component)]
pub struct SandboxHudRoot;

/// The page margin, cq.
pub const M: f32 = 2.6;

/// A gun as a HUD piece shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct GunFace {
    pub id: String,
    pub name: String,
    pub game_name: String,
    pub game_short: String,
    pub icon_class: Option<&'static str>,
}

impl GunFace {
    pub fn of(g: &SandboxGun) -> Self {
        GunFace { id: g.id.clone(), name: g.name.clone(), game_name: g.game_name.clone(), game_short: g.game_short.clone(), icon_class: g.icon_class }
    }

    /// A gun the catalogue doesn't list (its id's tail).
    pub fn bare(id: &str) -> Self {
        let (g, n) = id.split_once(':').unwrap_or(("", id));
        GunFace {
            id: id.into(),
            name: n.trim_start_matches("weapon_").replace('_', " "),
            game_name: super::game_name(g),
            game_short: super::game_short(g, None),
            icon_class: None,
        }
    }
}

/// What the HUD shows (rebuilt when it changes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudShown {
    pub on: bool,
    pub tab: bool,
    pub gun: Option<GunFace>,
    pub tags: Vec<&'static str>,
    pub next: Option<(GunFace, UnitState)>,
    /// The Next strip's gun is the mechanics' waiting pick.
    pub next_held: bool,
    /// The mechanics' last word (`SandboxUi::said`).
    pub said: Option<String>,
    pub pins: Vec<Option<GunFace>>,
    pub current_slot: Option<usize>,
    /// The bar's alpha, in steps (0: hidden).
    pub bar: u8,
    pub kills: Vec<KillCard>,
    /// Seconds since the last kill (whole).
    pub kill_age: Option<u64>,
    pub options: SandboxOptions,
    /// K's set-up to repeat ("2 targets, 1 prop"; None: nothing yet).
    pub repeat: Option<String>,
    pub size: (u32, u32),
}

/// What the HUD should show now.
pub fn shown(ui: &SandboxUi, cat: &SandboxCatalogue, progress: &UnitProgress, kills: &KillLog, options: &SandboxOptions, now: f64) -> HudShown {
    if !ui.on || ui.screen == SandboxScreen::ChangeGun {
        return HudShown::default();
    }
    let face = |id: &str| cat.get(id).map(GunFace::of).unwrap_or_else(|| GunFace::bare(id));
    let next = ui.pending.as_ref().map(|id| {
        let st = cat.get(id).map_or(UnitState::Here, |g| SandboxCatalogue::unit_state(g, progress));
        (face(id), st)
    });
    let held = ui.tab || ui.drag.is_some();
    let a = ui.pin_bar.alpha(now, held, FADE);
    HudShown {
        on: true,
        tab: ui.tab,
        gun: ui.current.as_deref().map(face),
        tags: options.tags(ui.tab),
        next_held: next.is_some() && ui.pick_held,
        said: ui.said.clone(),
        next,
        pins: ui.pins.0.iter().map(|p| p.as_deref().map(face)).collect(),
        current_slot: ui.current.as_deref().and_then(|c| ui.pins.slot_of(c)),
        bar: if ui.pins.any() || held { (a * ALPHA_STEPS).ceil() as u8 } else { 0 },
        kills: if ui.tab { kills.0.clone() } else { Vec::new() },
        kill_age: if ui.tab { kills.0.last().map(|k| (now - k.at).max(0.0) as u64) } else { None },
        options: *options,
        repeat: None,
        size: (0, 0),
    }
}

/// A gun's icon `h` cq tall in `tone` under `parent` (its game's own or
/// the kit's: `signal::gun_icon`).
pub fn gun_icon(c: &mut Commands, look: &Look, parent: Entity, id: &str, class: Option<&str>, h: f32, tone: Tone, alpha: f32) {
    let color = look.theme.tone_a(tone, alpha);
    let h = look.px(h);
    c.entity(parent).with_children(|p| {
        signal::gun_icon(p, look, id, class, h, color);
    });
}

/// A kit icon (`signal::icons`) `h` cq tall.
pub fn kit_icon(c: &mut Commands, look: &Look, parent: Entity, key: &str, h: f32, tone: Tone, alpha: f32) {
    let color = look.theme.tone_a(tone, alpha);
    let h = look.px(h);
    c.entity(parent).with_children(|p| {
        signal::icons::spawn(p, look, key, h, color);
    });
}

/// A small tag: Mono text in a box (`fill`: a solid fill, else an outline).
pub fn tag(c: &mut Commands, ui: &Ui, parent: Entity, text: &str, size: f32, fill: Option<(Tone, Tone)>, outline: Tone) -> Entity {
    let s = UiStyle { pad: Some([0.25 * size / 0.46, 0.4 * size / 0.46, 0.2 * size / 0.46, 0.4 * size / 0.46].into()), shrink: Some(0.0), ..UiStyle::row() };
    let (s, fg) = match fill {
        Some((bg, fg)) => (s.bg(bg), fg),
        None => (s.bg_a(Tone::Ink, 0.62).outline(outline, 1.0), Tone::Fg),
    };
    let b = c.spawn((s, Node::default(), Pickable::IGNORE, ChildOf(parent))).id();
    ui.text(c, b, Role::Mono, size, fg, text);
    b
}

/// A key cap and its words (the hints, the keys card).
pub fn key_hint(c: &mut Commands, ui: &Ui, parent: Entity, key: &str, words: &str, size: f32, tone: Tone) -> Entity {
    let g = ui.boxed(c, parent, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), ..UiStyle::row() });
    ui.kbd(c, g, key, size, tone);
    ui.text(c, g, Role::Mono, size, tone, words);
    g
}

fn spawn_root(mut c: Commands, layer: Option<Res<super::super::overlay::OverlayLayer>>) {
    let mut e = c.spawn((
        SandboxHudRoot,
        Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        GlobalZIndex(crate::client::minigame_scoreboard::SCOREBOARD_Z + 1),
        Pickable::IGNORE,
        Visibility::Hidden,
    ));
    if let Some(l) = layer {
        e.insert(ChildOf(l.0));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    cat: Res<SandboxCatalogue>,
    progress: Res<UnitProgress>,
    kills: Res<KillLog>,
    options: Res<SandboxOptions>,
    clock: Option<Res<chimera_ui::UiClock>>,
    layout: Option<Res<chimera_sim::minigame::layout::SandboxLayout>>,
    mut root: Query<(Entity, &mut Visibility), With<SandboxHudRoot>>,
    mut last: Local<HudShown>,
    cover: Option<Res<crate::client::hud::HudCover>>,
    mut c: Commands,
) {
    let Ok((root, mut vis)) = root.single_mut() else { return };
    // `cl_drawhud 0`: none of it (reference captures).
    let off = cover.is_some_and(|c| c.is(crate::client::hud::DRAWHUD_COVER));
    let now = clock.map_or(0.0, |c| c.now);
    let mut next = if look.on() { shown(&ui_state, &cat, &progress, &kills, &options, now) } else { HudShown::default() };
    next.size = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    next.repeat = layout.filter(|l| !l.is_empty()).map(|l| l.summary.split(':').next().unwrap_or("").trim().to_string());
    if ui_state.staged && next.on {
        next.repeat = Some("2 targets, 1 prop".into());
    }
    let want = if next.on && !off { Visibility::Inherited } else { Visibility::Hidden };
    if *last == next {
        vis.set_if_neq(want);
        return;
    }
    *last = next.clone();
    c.entity(root).despawn_children();
    vis.set_if_neq(want);
    if !next.on {
        return;
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    top_left(&mut c, &ui, root, next.tab);
    chip(&mut c, &ui, &look, root, &next);
    if next.tab {
        tab_column(&mut c, &ui, &look, root, &next);
    }
    if next.bar > 0 {
        let a = next.bar as f32 / ALPHA_STEPS;
        let bar = c
            .spawn((
                UiStyle { flow: chimera_ui::style::Flow::Column, align: Some(AlignItems::Center), ..default() }.abs([Some(0.0), None, Some(0.0), Some(7.4)]),
                Node::default(),
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id();
        pin_bar(&mut c, &ui, &look, bar, &next.pins, next.current_slot, None, 1.0, a);
    }
}

fn top_left(c: &mut Commands, ui: &Ui, root: Entity, tab: bool) {
    let col = c
        .spawn((
            UiStyle { gap: Some(0.65), align: Some(AlignItems::FlexStart), ..UiStyle::column() }.abs([Some(M), Some(1.9), None, None]),
            Node::default(),
            UiParityId::new("sb.mark"),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let line = ui.boxed(c, col, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
    let mark = ui.boxed(c, line, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
    ui.mark(c, mark, Tone::Acc, 0.42, 0.75);
    ui.text(c, mark, Role::DisplayBold, 0.9, Tone::Fg, "Sandbox");
    if tab {
        let held = ui.boxed(c, line, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), margin: Some([0.0, 0.0, 0.0, 3.4].into()), ..UiStyle::row() });
        let cap = ui.boxed(c, held, UiStyle { pad: Some([0.2, 0.3, 0.2, 0.3].into()), ..UiStyle::row() }.bg(Tone::Fg));
        ui.text(c, cap, Role::Mono, 0.54 * 0.82, Tone::Ink, "Tab");
        ui.text(c, held, Role::Mono, 0.54, Tone::Fg, "Held");
    }
    let keys = ui.boxed(c, col, UiStyle { gap: Some(1.0), ..UiStyle::row() });
    key_hint(c, ui, keys, "B", "Change gun", 0.54, Tone::Fg);
    key_hint(c, ui, keys, "M", "Change map", 0.54, Tone::Fg);
    key_hint(c, ui, keys, "T", "Bench", 0.54, Tone::Fg);
}

/// The chip (module doc), centred at the top.
fn chip(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, s: &HudShown) {
    let Some(gun) = &s.gun else { return };
    let col = c
        .spawn((
            UiStyle { gap: Some(0.0), align: Some(AlignItems::Center), ..UiStyle::column() }.abs([Some(0.0), Some(1.9), Some(0.0), None]),
            Node::default(),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    gun_chip(c, ui, look, col, gun);
    let under = c
        .spawn((UiStyle { gap: Some(0.3), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }, Node::default(), UiParityId::new("sb.conds"), ChildOf(col)))
        .id();
    match &s.next {
        Some((g, st)) => next_strip(c, ui, look, under, g, st, s.next_held),
        None => {
            for t in &s.tags {
                tag(c, ui, under, t, 0.46, None, Tone::Fg4);
            }
        }
    }
    if let Some(said) = &s.said {
        let r = c
            .spawn((
                UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), margin: Some([0.4, 0.0, 0.0, 0.0].into()), pad: Some([0.35, 0.7, 0.35, 0.7].into()), ..UiStyle::row() }.bg_a(Tone::Ink, 0.86),
                Node::default(),
                UiParityId::new("sb.said"),
                ChildOf(col),
            ))
            .id();
        ui.mark(c, r, Tone::Threat, 0.3, 0.6);
        ui.sentence(c, r, 0.6, Tone::Fg, said);
    }
}

/// The gun chip itself (also Change gun's sheet A's head).
pub fn gun_chip(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, gun: &GunFace) -> Entity {
    let plate = c
        .spawn((
            UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), pad: Some([0.55, 1.2, 0.55, 1.0].into()), min_w: Some(12.0), shrink: Some(0.0), ..UiStyle::row() }
                .bg_a(Tone::Ink, 0.86)
                .outline(Tone::Fg5, 1.0),
            Node::default(),
            UiParityId::new("sb.chip"),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    let icon = ui.boxed(c, plate, UiStyle { w: Some(3.4), h: Some(1.9), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
    gun_icon(c, look, icon, &gun.id, gun.icon_class, 1.15, Tone::Fg, 1.0);
    let words = ui.boxed(c, plate, UiStyle { gap: Some(0.32), ..UiStyle::column() });
    let n = ui.text(c, words, Role::DisplayBold, 1.02, Tone::Fg, &gun.name);
    c.entity(n).insert(UiParityId::new("sb.chip.name"));
    let g = ui.text(c, words, Role::Mono, 0.48, Tone::Fg3, &gun.game_name);
    c.entity(g).insert(UiParityId::new("sb.chip.game"));
    plate
}

/// The progress component's words for a download state.
pub fn progress_of(st: &UnitState) -> (chimera_ui::controls::Progress, String) {
    use chimera_ui::controls::Progress;
    match st {
        UnitState::Downloading { done, total } => (
            Progress::Running(st.share()),
            format!("{:.1} of {}", *done as f64 / 1e6, size_words(*total)),
        ),
        UnitState::Queued { .. } => (Progress::Pending, "Queued".into()),
        UnitState::Failed { .. } => (Progress::Pending, "Failed".into()),
        UnitState::NotHere { bytes } => (Progress::Pending, size_words(*bytes)),
        UnitState::Here => (Progress::Ready, "Here".into()),
    }
}

fn next_strip(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, g: &GunFace, st: &UnitState, held: bool) {
    let strip = c
        .spawn((
            UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), pad: Some([0.4, 0.7, 0.4, 0.7].into()), ..UiStyle::row() }.bg_a(Tone::Ink, 0.86).outline(Tone::Fg5, 1.0),
            Node::default(),
            UiParityId::new("sb.next"),
            ChildOf(parent),
        ))
        .id();
    ui.text(c, strip, Role::Mono, 0.46, Tone::Fg3, "Next");
    gun_icon(c, look, strip, &g.id, g.icon_class, 0.62, Tone::Fg, 1.0);
    ui.text(c, strip, Role::DisplayBold, 0.66, Tone::Fg, &g.name);
    // The mechanics wait for files the units' list calls here: loading.
    let (p, words) = if held && st.is_here() { (chimera_ui::controls::Progress::Pending, "Loading".into()) } else { progress_of(st) };
    ui.progress(c, strip, p, &words, 10.4, 1.1, 0.44, true);
    if matches!(st, UnitState::Failed { .. }) {
        key_hint(c, ui, strip, "R", "Retry", 0.46, Tone::Fg);
    }
}

/// The pinned bar under `parent` (module doc): slots `w` cq wide (1:
/// 6.4 x 3.6; smaller in Change gun), `drop` the slot a drag would drop on
/// and the dragged gun: a ghost of it there, faint in an accent frame,
/// the gun it replaces struck through under the slot (the design rules,
/// section 22, drop marker C).
#[allow(clippy::too_many_arguments)]
pub fn pin_bar(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, pins: &[Option<GunFace>], cur: Option<usize>, drop: Option<(usize, GunFace)>, scale: f32, alpha: f32) -> Vec<Entity> {
    let rack = c
        .spawn((
            UiStyle { ..UiStyle::row() }.bg_a(Tone::Ink, 0.82 * alpha).outline(Tone::Fg4, alpha),
            Node::default(),
            UiParityId::new("sb.pins"),
            ChildOf(parent),
        ))
        .id();
    let (w, h) = (6.4 * scale, 3.6 * scale);
    let mut out = Vec::new();
    for i in 0..PIN_SLOTS {
        let gun = pins.get(i).cloned().flatten();
        let on = cur == Some(i) && gun.is_some();
        let ghost = drop.as_ref().filter(|d| d.0 == i).map(|d| &d.1);
        let dropping = ghost.is_some();
        let mut s = UiStyle {
            w: Some(w),
            h: Some(h),
            gap: Some(0.35 * scale),
            pad: Some([0.0, 0.0, 0.5 * scale, 0.0].into()),
            align: Some(AlignItems::Center),
            justify: Some(if gun.is_some() || dropping { JustifyContent::FlexEnd } else { JustifyContent::Center }),
            shrink: Some(0.0),
            ..UiStyle::column()
        };
        if on {
            s = s.bg_a(Tone::Fg, alpha);
        }
        if dropping {
            s = s.bg_a(Tone::Acc, 0.12 * alpha);
        }
        let slot = c.spawn((s, Node::default(), UiParityId::new(format!("sb.pin.{}", i + 1)), ChildOf(rack))).id();
        if i > 0 && !on {
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(look.hair()), ..default() },
                BackgroundColor(look.theme.tone_a(Tone::Fg5, alpha)),
                Pickable::IGNORE,
                ChildOf(slot),
            ));
        }
        let fg = if on { Tone::Ink } else { Tone::Fg };
        // The key cap, top left; the game, top right.
        let cap = c
            .spawn((
                UiStyle { pad: Some([0.12, 0.24, 0.12, 0.24].into()), ..UiStyle::row() }.abs([Some(0.3 * scale), Some(0.3 * scale), None, None]).outline(fg, alpha),
                Node::default(),
                Pickable::IGNORE,
                ChildOf(slot),
            ))
            .id();
        ui.text_a(c, cap, Role::Mono, 0.44 * scale.max(0.8), fg, alpha, &(i + 1).to_string());
        if let Some(g) = ghost {
            // The frame: an accent border, broken into dashes along each
            // side.
            dashed_frame(c, look, slot, alpha);
            let gm = ui.boxed(c, slot, UiStyle::row().abs([None, Some(0.38 * scale), Some(0.35 * scale), None]));
            ui.text_a(c, gm, Role::Mono, 0.4 * scale.max(0.8), Tone::AccText, alpha, &g.game_short);
            gun_icon(c, look, slot, &g.id, g.icon_class, 1.05 * scale, Tone::Fg, 0.45 * alpha);
            ui.text_a(c, slot, Role::Mono, 0.46 * scale.max(0.85), Tone::AccText, alpha, &g.name);
            if let Some(old) = gun.as_ref().filter(|o| o.id != g.id) {
                let was = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, top: Val::Percent(100.0), margin: UiRect::top(look.val(0.3)), ..default() },
                        Pickable::IGNORE,
                        ChildOf(slot),
                    ))
                    .id();
                ui.text_a(c, was, Role::Mono, 0.4 * scale.max(0.8), Tone::Fg3, alpha, &format!("Was {}", old.name));
                c.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::Percent(50.0), height: Val::Px(look.hair()), ..default() },
                    BackgroundColor(look.theme.tone_a(Tone::Fg3, alpha)),
                    UiParityId::new("sb.pin.was"),
                    Pickable::IGNORE,
                    ChildOf(was),
                ));
            }
        } else if let Some(g) = &gun {
            let gm = ui.boxed(c, slot, UiStyle::row().abs([None, Some(0.38 * scale), Some(0.35 * scale), None]));
            ui.text_a(c, gm, Role::Mono, 0.4 * scale.max(0.8), if on { Tone::Ink } else { Tone::Fg3 }, if on { 0.6 * alpha } else { alpha }, &g.game_short);
            gun_icon(c, look, slot, &g.id, g.icon_class, 1.05 * scale, fg, alpha);
            ui.text_a(c, slot, Role::Mono, 0.46 * scale.max(0.85), fg, alpha, &g.name);
        } else {
            ui.text_a(c, slot, Role::Mono, 0.48 * scale.max(0.8), Tone::Fg4, alpha, "Empty");
        }
        out.push(slot);
    }
    out
}

/// An accent frame of dashes round `parent` (the drop marker).
fn dashed_frame(c: &mut Commands, look: &Look, parent: Entity, alpha: f32) {
    let (dash, gap) = (6.0, 4.0);
    let hair = (2.0 * look.units.scale_factor).round() / look.units.scale_factor;
    let color = look.theme.tone_a(Tone::Acc, alpha);
    let frame = c
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, overflow: Overflow::clip(), ..default() },
            UiParityId::new("sb.pin.drop"),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();
    // Enough dashes for a slot up to 200 px wide and 120 tall.
    for k in 0..20 {
        let at = Val::Px(k as f32 * (dash + gap));
        for (left, top, w, h, bottom, right) in [
            (Some(at), Some(Val::ZERO), dash, hair, None, None),
            (Some(at), None, dash, hair, Some(Val::ZERO), None),
            (Some(Val::ZERO), Some(at), hair, dash, None, None),
            (None, Some(at), hair, dash, None, Some(Val::ZERO)),
        ] {
            if k >= 12 && w < h {
                continue;
            }
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: left.unwrap_or(Val::Auto),
                    top: top.unwrap_or(Val::Auto),
                    bottom: bottom.unwrap_or(Val::Auto),
                    right: right.unwrap_or(Val::Auto),
                    width: Val::Px(w),
                    height: Val::Px(h),
                    ..default()
                },
                BackgroundColor(color),
                Pickable::IGNORE,
                ChildOf(frame),
            ));
        }
    }
}

fn card(c: &mut Commands, parent: Entity, id: &'static str, pad: [f32; 4], gap: f32) -> Entity {
    c.spawn((
        UiStyle { gap: Some(gap), pad: Some(pad.into()), ..UiStyle::column() }.bg_a(Tone::Ink, 0.84).outline(Tone::Fg5, 1.0),
        Node::default(),
        UiParityId::new(id),
        Pickable::IGNORE,
        ChildOf(parent),
    ))
    .id()
}

fn motion_words(m: TargetMotion) -> &'static str {
    match m {
        TargetMotion::Still => "Standing still",
        TargetMotion::Strafes => "Strafing",
        TargetMotion::Walks => "Walking",
    }
}

/// The Tab column (module doc).
fn tab_column(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, s: &HudShown) {
    let col = c
        .spawn((
            UiStyle { gap: Some(0.6), w: Some(21.0), ..UiStyle::column() }.abs([Some(M), Some(5.6), None, None]),
            Node::default(),
            UiParityId::new("sb.tab"),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    if let Some(k) = s.kills.last() {
        kill_card(c, ui, look, col, k, &s.kills[..s.kills.len() - 1], s.kill_age.unwrap_or(0));
    }
    // Options.
    let o = card(c, col, "sb.options", [0.8, 1.0, 0.8, 1.0], 0.3);
    let head = ui.boxed(c, o, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), margin: Some([0.0, 0.0, 0.3, 0.0].into()), ..UiStyle::row() });
    tag(c, ui, head, "Options", 0.46, Some((Tone::Fg, Tone::Ink)), Tone::Fg);
    ui.boxed(c, head, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    key_hint(c, ui, head, "T", "changes them", 0.46, Tone::Fg3);
    for opt in SandboxOption::ALL {
        let on = s.options.get(opt);
        let tone = if on { Tone::Fg } else { Tone::Fg3 };
        let r = ui.boxed(c, o, UiStyle { h: Some(1.45), justify: Some(JustifyContent::SpaceBetween), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.hairline(c, r, Tone::Fg5, 1.0);
        ui.text(c, r, Role::DisplayRegular, 0.64, tone, opt.label());
        ui.text(c, r, Role::Mono, 0.48, tone, if on { "On" } else { "Off" });
    }
    // Keys.
    let k = card(c, col, "sb.keys", [0.8, 1.0, 0.8, 1.0], 0.5);
    let head = ui.boxed(c, k, UiStyle { gap: Some(0.6), ..UiStyle::row() });
    tag(c, ui, head, "Keys", 0.46, Some((Tone::Fg, Tone::Ink)), Tone::Fg);
    let grid = ui.boxed(c, k, UiStyle { gap: Some(0.45), ..UiStyle::column() });
    let repeat = s.repeat.as_ref().map_or("Repeat: nothing yet".to_string(), |r| format!("Repeat: {r}"));
    for pair in [[("B", "Change gun".to_string()), ("M", "Change map".to_string())], [("T", "Bench".to_string()), ("1-6", "Pinned guns".to_string())], [("K", repeat), ("Esc", "Menu".to_string())]] {
        let row = ui.boxed(c, grid, UiStyle::row());
        for (key, words) in pair {
            if key.is_empty() {
                continue;
            }
            let half = ui.boxed(c, row, UiStyle { w_pct: Some(50.0), ..UiStyle::row() });
            // K is greyed until there is a set-up to repeat.
            let tone = if key == "K" && s.repeat.is_none() { Tone::Fg4 } else { Tone::Fg };
            key_hint(c, ui, half, key, &words, 0.5, tone);
        }
    }
}

fn kill_card(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, k: &KillCard, earlier: &[KillCard], age: u64) {
    let card = card(c, parent, "sb.kill", [0.9, 1.0, 1.0, 1.0], 0.6);
    let head = ui.boxed(c, card, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
    tag(c, ui, head, "Last kill", 0.46, Some((Tone::Acc, Tone::OnAcc)), Tone::Acc);
    ui.text(c, head, Role::Mono, 0.46, Tone::Fg3, &format!("{age} s ago"));
    ui.boxed(c, head, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, head, Role::Mono, 0.46, Tone::Fg3, "K repeats the set-up");
    let what = ui.boxed(c, card, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
    gun_icon(c, look, what, &k.gun, None, 0.8, Tone::Fg, 1.0);
    ui.text(c, what, Role::DisplayBold, 0.85, Tone::Fg, &k.gun_name);
    ui.text(c, what, Role::Mono, 0.46, Tone::Fg3, &k.gun_game_short);
    ui.text(c, what, Role::DisplayBold, 0.85, Tone::Fg3, ">");
    ui.text(c, what, Role::DisplayBold, 0.85, Tone::Fg, &k.victim);
    ui.text(c, card, Role::Mono, 0.52, Tone::Fg2, &format!("{} \u{00b7} {:.0} m \u{00b7} {}", k.protection, k.distance_m, motion_words(k.motion)));
    // The three numbers.
    let stats = ui.boxed(c, card, UiStyle { gap: Some(0.6), pad: Some([0.3, 0.0, 0.3, 0.0].into()), ..UiStyle::row() });
    for (label, value, unit, w) in [
        ("Hits", k.hits.len().to_string(), format!("of {} shots", k.shots), 26.0),
        ("First hit to kill", format!("{:.2}", k.first_hit_to_kill), "s".to_string(), 39.0),
        ("Distance", format!("{:.1}", k.distance_m), "m".to_string(), 35.0),
    ] {
        let s = ui.boxed(c, stats, UiStyle { gap: Some(0.3), w_pct: Some(w), ..UiStyle::column() });
        ui.text(c, s, Role::Mono, 0.44, Tone::Fg3, label);
        ui.text(c, s, Role::Mono, 1.9, Tone::Fg, &value);
        ui.text(c, s, Role::Mono, 0.5, Tone::Fg3, &unit);
    }
    // The hits, as slanted ticks along a line.
    let strip = ui.boxed(c, card, UiStyle { h: Some(3.4), margin: Some([0.0, 0.6, 0.0, 0.6].into()), ..UiStyle::row() });
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: look.val(0.55), height: Val::Px(look.hair()), ..default() },
        BackgroundColor(look.theme.tone(Tone::Fg4)),
        ChildOf(strip),
    ));
    let total = k.first_hit_to_kill.max(1e-3);
    let n = k.hits.len();
    for (i, h) in k.hits.iter().enumerate() {
        let last = i + 1 == n;
        let pct = if n <= 1 { 50.0 } else { (h.t / total * 100.0).clamp(0.0, 100.0) };
        let hm = c
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(pct),
                    top: Val::ZERO,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: look.val(0.2),
                    ..default()
                },
                UiTransform { translation: Val2::percent(-50.0, 0.0), ..default() },
                ChildOf(strip),
            ))
            .id();
        ui.mark(c, hm, if last { Tone::Fg } else { Tone::Fg2 }, 0.38, 1.1);
        ui.text(c, hm, Role::Mono, 0.5, Tone::Fg, &format!("{:.0}", h.damage));
        ui.text(c, hm, Role::Mono, 0.4, if last { Tone::Fg } else { Tone::Fg3 }, &h.group);
    }
    if !earlier.is_empty() {
        let e = ui.boxed(c, card, UiStyle { pad: Some([0.5, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(look.hair()), ..default() },
            BackgroundColor(look.theme.tone(Tone::Fg5)),
            ChildOf(e),
        ));
        let l = ui.boxed(c, e, UiStyle { margin: Some([0.0, 0.0, 0.3, 0.0].into()), ..UiStyle::row() });
        ui.text(c, l, Role::Mono, 0.44, Tone::Fg3, "Earlier");
        for k in earlier.iter().rev() {
            let r = ui.boxed(c, e, UiStyle { h: Some(1.35), gap: Some(0.4), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.hairline(c, r, Tone::Fg5, 1.0);
            let ic = ui.boxed(c, r, UiStyle { w: Some(1.8), shrink: Some(0.0), ..UiStyle::row() });
            gun_icon(c, look, ic, &k.gun, None, 0.5, Tone::Fg2, 1.0);
            let nm = ui.boxed(c, r, UiStyle { w: Some(5.6), shrink: Some(0.0), ..UiStyle::row() });
            ui.text(c, nm, Role::DisplayBold, 0.58, Tone::Fg, &k.gun_name);
            let pr = ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
            ui.text(c, pr, Role::Mono, 0.48, Tone::Fg2, &k.protection);
            for (w, t) in [(1.5, format!("{:.0} m", k.distance_m)), (2.3, format!("{} hits", k.hits.len())), (2.0, format!("{:.2} s", k.first_hit_to_kill))] {
                let b = ui.boxed(c, r, UiStyle { w: Some(w), justify: Some(JustifyContent::FlexEnd), shrink: Some(0.0), ..UiStyle::row() });
                ui.text(c, b, Role::Mono, 0.48, Tone::Fg2, &t);
            }
        }
    }
}

/// The HUD's systems.
pub fn add(app: &mut App) {
    super::super::overlay::layer(app);
    app.add_systems(Startup, spawn_root).add_systems(Update, draw.after(super::SandboxSystems));
}
