//! The rank-up toast (docs/plans/active/ugg-ui.md, M3): under the Signal
//! look (`signal::Look::on`, the race chip's `RaceView::active`), each of
//! the local player's rung ups shows a small toast under the top-centre
//! chip (`layout.toast`: 6.8 cq down, 3.1 cq tall), in and out in about
//! 1.15 s (`motion.dur.toast`):
//!
//! - the plate wipes in from the left along the slant (0.2 s, `expo_out`);
//! - RUNG and the number rolling upward: the old one out through the top,
//!   the new one up from below with a slight overshoot (from 0.08 s over
//!   0.3 s, `back(.9)`);
//! - the new gun wiping in (from 0.12 s over 0.28 s): its icon (its
//!   game's own where it has one, else the kit's by class:
//!   `signal::gun_icon`, `icons::choose`), line 1 its short name
//!   (`minigame_scoreboard::gun_short_name`, fitted: `overlay::fit`),
//!   line 2 always class and game ("SMG • CS:S");
//! - the next gun on the right (from 0.2 s over 0.25 s);
//! - held to 0.92 s, then cut away from the left (`expo_in`, 0.24 s).
//!
//! A rung up within the hold or the exit **chains**: the toast stays,
//! the number rolls again, the old gun lifts out as the new one wipes in,
//! the next gun swaps. The chip's own rung (`Roll`) and its meter's
//! current tick (`TickPop`, a pop in height) move in step with the roll.
//! The pose is a function of the recent rung ups and the UI clock
//! (`pose`), so a frame at any time is reproducible (tests step
//! `UiClock::manual`).
//!
//! The rung-up callout (`callouts`) shows no text under the Signal look
//! (its cue still sounds): the toast says it. Other callouts stay.
//! `ui_scene rankup [t]` stages a loop (a single roll, then a chain of
//! two) on the staged race chip; with `t` the clock is frozen at `t` s
//! into the loop (screenshots).
//!
//! Known shortcuts: the plate's content is clipped upright while its
//! fill's cut leans with the slant (the 0.66 cq lean shows on the fill
//! only); no text glow (Bevy has no text blur).

use std::collections::VecDeque;

use bevy::prelude::*;
use chimera_sim::{
    minigame::{ladder::{Ladder, Rung}, runtime::InMinigame},
    weapon::data::{WeaponCatalog, WeaponData},
};
use chimera_ui::{Ease, Role, SlantMaterial, Tone, UiClock};

use super::{
    minigame_scoreboard::{GunNames, SCOREBOARD_Z, gun_short_name},
    race_chip::{RaceStaged, RaceView, STAGED_GUNS},
    signal::{self, Look, TagKind, UiOpacity},
};
use crate::core::LocalPlayer;

/// The toast and its watch.
pub struct RankToastPlugin;

impl Plugin for RankToastPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RankToast>().add_systems(Startup, spawn).add_systems(
            Update,
            (run_script, watch, build, animate).chain().after(super::race_chip::RaceChipSystems),
        );
    }
}

/// A gun as the toast shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToastGun {
    pub id: Option<&'static str>,
    /// Its short name (line 1).
    pub name: String,
    /// Class and game: "SMG • CS:S" (line 2).
    pub line2: String,
    /// The kit's icon by class (`signal::icon_class`).
    pub class: Option<&'static str>,
}

/// One rung up of the local player.
#[derive(Clone, Debug, PartialEq)]
pub struct RankEvent {
    /// When, on the `UiClock`.
    pub at: f64,
    /// Rungs, from 0.
    pub from: usize,
    pub to: usize,
    pub gun: ToastGun,
    /// The rung after `to`'s gun (none on the last).
    pub next: Option<ToastGun>,
}

/// The local player's recent rung ups (the last few: a chain looks back
/// one) and the rung last seen.
#[derive(Resource, Clone, Debug, Default)]
pub struct RankToast {
    pub events: VecDeque<RankEvent>,
    pub seen: Option<usize>,
}

impl RankToast {
    pub fn push(&mut self, e: RankEvent) {
        self.events.push_back(e);
        while self.events.len() > 4 {
            self.events.pop_front();
        }
    }

    /// The toast's pose at `now` (`pose`).
    pub fn pose(&self, now: f64, d: &ToastTimes) -> Option<ToastPose> {
        pose(self.events.iter().collect(), now, d)
    }
}

/// The toast's times (`motion.dur.toast`, the look book's by default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToastTimes {
    pub wipe: f32,
    pub roll_at: f32,
    pub roll: f32,
    pub gun_at: f32,
    pub gun: f32,
    pub next_at: f32,
    pub next: f32,
    pub hold_to: f32,
    pub leave: f32,
    pub roll_ease: Ease,
}

impl Default for ToastTimes {
    fn default() -> Self {
        ToastTimes {
            wipe: 0.2,
            roll_at: 0.08,
            roll: 0.3,
            gun_at: 0.12,
            gun: 0.28,
            next_at: 0.2,
            next: 0.25,
            hold_to: 0.92,
            leave: 0.24,
            roll_ease: Ease::Back(0.9),
        }
    }
}

impl ToastTimes {
    pub fn of(look: &Look) -> ToastTimes {
        let d = ToastTimes::default();
        let t = |k: &str, or: f32| look.dur("toast", k, or);
        ToastTimes {
            wipe: t("wipe", d.wipe),
            roll_at: t("roll_at", d.roll_at),
            roll: t("roll", d.roll),
            gun_at: t("gun_at", d.gun_at),
            gun: t("gun", d.gun),
            next_at: t("next_at", d.next_at),
            next: t("next", d.next),
            hold_to: t("hold_to", d.hold_to),
            leave: t("leave", d.leave),
            roll_ease: look.theme.ease("roll"),
        }
    }

    /// In to out, one rung up: 1.16 s by default.
    pub fn total(&self) -> f32 {
        self.hold_to + self.leave
    }
}

/// On a chain: the old gun lifts out over this, the new one wipes in from
/// `CHAIN_GUN_AT`, the next gun swaps at `CHAIN_NEXT_SWAP` (fading out
/// and back in over as long).
const CHAIN_LIFT: f32 = 0.16;
const CHAIN_GUN_AT: f32 = 0.07;
const CHAIN_NEXT_SWAP: f32 = 0.12;

fn c01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn e_out(p: f32) -> f32 {
    Ease::ExpoOut.sample(c01(p))
}

fn e_in(p: f32) -> f32 {
    Ease::ExpoIn.sample(c01(p))
}

/// Where everything in the toast is at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct ToastPose {
    /// The rung up showing and the first of its chain (indices into the
    /// events).
    pub k: usize,
    pub j: usize,
    /// Seconds since the rung up showing.
    pub dt: f32,
    /// The wipe in (0-1, the chain's first) and out (0-1).
    pub enter: f32,
    pub exit: f32,
    /// The roll's progress (0-1) and its eased value (overshoots).
    pub roll: f32,
    pub rolled: f32,
    /// The old number's and the new one's offsets, in roll heights (up
    /// negative), and the old one's opacity.
    pub old_y: f32,
    pub old_alpha: f32,
    pub new_y: f32,
    /// The chip's current tick's height (1: at rest).
    pub pop: f32,
    /// The new gun's wipe (0-1).
    pub gun: f32,
    /// On a chain: the old gun's lift (in slot heights, up negative) and
    /// opacity.
    pub old_gun: Option<(f32, f32)>,
    /// Which rung up's next gun shows, and its opacity.
    pub next_of: usize,
    pub next_alpha: f32,
}

impl ToastPose {
    pub fn visible(&self) -> bool {
        self.enter > 0.0 && self.exit < 1.0
    }

    /// The roll is under way (the chip's number and tick move).
    pub fn rolling(&self) -> bool {
        self.roll > 0.0 && self.roll < 1.0
    }
}

/// The toast at `now` for rung ups `events` (oldest first): the look
/// book's choreography (module doc); `None` before the first or once the
/// last has left.
pub fn pose(events: Vec<&RankEvent>, now: f64, d: &ToastTimes) -> Option<ToastPose> {
    let k = events.iter().rposition(|e| e.at <= now)?;
    // The chain: back while each came before the one before it left.
    let mut j = k;
    while j > 0 && events[j].at < events[j - 1].at + d.total() as f64 {
        j -= 1;
    }
    let dt = (now - events[k].at) as f32;
    let enter = e_out((now - events[j].at) as f32 / d.wipe);
    let exit = e_in((dt - d.hold_to) / d.leave);
    if exit >= 1.0 {
        return None;
    }
    let roll = c01((dt - d.roll_at) / d.roll);
    let rolled = d.roll_ease.sample(roll);
    let chained = k != j;
    let (gun, old_gun) = if chained {
        let o = c01(dt / CHAIN_LIFT);
        (e_out((dt - CHAIN_GUN_AT) / d.gun), Some((-e_out(o) * 0.45, 1.0 - o)))
    } else {
        (e_out((dt - d.gun_at) / d.gun), None)
    };
    let (next_of, next_alpha) = if chained {
        let swapped = dt >= CHAIN_NEXT_SWAP;
        (if swapped { k } else { k - 1 }, c01((dt - CHAIN_NEXT_SWAP).abs() / CHAIN_NEXT_SWAP))
    } else {
        (k, e_out((dt - d.next_at) / d.next))
    };
    Some(ToastPose {
        k,
        j,
        dt,
        enter,
        exit,
        roll,
        rolled,
        old_y: -rolled * 1.1,
        old_alpha: 1.0 - c01(roll * 1.6) * 0.7,
        new_y: (1.0 - rolled) * 1.1,
        pop: 1.0 + 0.9 * (std::f32::consts::PI * roll).sin(),
        gun,
        old_gun,
        next_of,
        next_alpha,
    })
}

/// A gun class's name on line 2 (`GunClass`; melee by its slot).
pub fn class_label(data: &WeaponData) -> &'static str {
    use chimera_core::game_parts::GunClass;
    if data.melee.is_some() {
        return "Melee";
    }
    match data.class {
        Some(GunClass::Pistol) => "Pistol",
        Some(GunClass::Smg) => "SMG",
        Some(GunClass::Shotgun) => "Shotgun",
        Some(GunClass::Carbine) => "Carbine",
        Some(GunClass::Rifle) => "Rifle",
        Some(GunClass::Heavy) => "Machine gun",
        Some(GunClass::Marksman) => "Marksman",
        Some(GunClass::Sniper) => "Sniper",
        Some(GunClass::HeavyPistol) => "Heavy pistol",
        Some(GunClass::Launcher) => "Launcher",
        Some(GunClass::Flamethrower) => "Flamethrower",
        Some(GunClass::Grenade) => "Grenade",
        None => "Gun",
    }
}

/// Gun `id` as the toast shows it.
pub fn toast_gun(
    id: &'static str,
    names: Option<&GunNames>,
    catalog: Option<&WeaponCatalog>,
    games: Option<&chimera_core::game_registry::Games>,
) -> ToastGun {
    let data = catalog.and_then(|c| c.get(id));
    let g = id.split_once(':').map_or("", |(g, _)| g);
    let game = games.and_then(|r| r.get(g)).map_or_else(|| g.to_uppercase(), |i| i.short_name.to_string());
    let class = data.map_or("Gun", class_label);
    ToastGun {
        id: Some(id),
        name: gun_short_name(id, names, catalog),
        line2: format!("{class} • {game}"),
        class: data.and_then(signal::icon_class),
    }
}

/// The staged race's gun on rung `rung` (`race_chip::STAGED_GUNS`).
pub fn staged_gun(rung: usize) -> ToastGun {
    let (id, name, game, class) = STAGED_GUNS[rung.min(STAGED_GUNS.len() - 1)];
    let label = match class {
        "smg" => "SMG".to_string(),
        "lmg" => "Machine gun".to_string(),
        "energy" => "Energy".to_string(),
        "revolver" => "Heavy pistol".to_string(),
        "knife" => "Melee".to_string(),
        c => {
            let mut s = c.to_string();
            s[..1].make_ascii_uppercase();
            s
        }
    };
    ToastGun { id: Some(id), name: name.to_string(), line2: format!("{label} • {game}"), class: Some(class) }
}

/// A staged loop (`ui_scene rankup`): rung ups at `steps` (seconds into
/// the loop, the rung reached) every `period` s from `start`, the staged
/// race's local rung following.
#[derive(Resource, Clone, Debug)]
pub struct RankScript {
    pub start: f64,
    pub period: f32,
    pub base: usize,
    pub steps: Vec<(f32, usize)>,
    cycle: Option<i64>,
}

/// The loop `ui_scene rankup` plays: 06 to 07 alone, then 07 to 08 and
/// 08 to 09 chained (0.42 s apart), every 4.4 s.
pub fn rankup_script(start: f64) -> RankScript {
    RankScript { start, period: 4.4, base: 6, steps: vec![(0.3, 7), (2.3, 8), (2.72, 9)], cycle: None }
}

/// Stages the loop from `start` (clock time), or clears it.
pub fn stage(world: &mut World, start: Option<f64>) {
    match start {
        Some(s) => {
            super::race_chip::stage(world, Some("chip"));
            world.insert_resource(rankup_script(s));
            *world.resource_mut::<RankToast>() = RankToast::default();
        }
        None => {
            if world.remove_resource::<RankScript>().is_some() {
                *world.resource_mut::<RankToast>() = RankToast::default();
                super::race_chip::stage(world, None);
            }
        }
    }
}

/// The staged loop's rung ups and the staged chip's rung.
fn run_script(clock: Res<UiClock>, script: Option<ResMut<RankScript>>, staged: Option<ResMut<RaceStaged>>, mut toast: ResMut<RankToast>) {
    let (Some(mut script), Some(mut staged)) = (script, staged) else { return };
    let t = (clock.now - script.start).max(0.0);
    let cycle = (t / script.period as f64).floor() as i64;
    if script.cycle != Some(cycle) {
        script.cycle = Some(cycle);
        toast.events.clear();
        let begin = script.start + cycle as f64 * script.period as f64;
        let mut from = script.base;
        for &(off, to) in &script.steps {
            toast.push(RankEvent {
                at: begin + off as f64,
                from,
                to,
                gun: staged_gun(to),
                next: Some(staged_gun(to + 1)),
            });
            from = to;
        }
    }
    let rung = toast.events.iter().rev().find(|e| e.at <= clock.now).map_or(script.base, |e| e.to);
    if let Some(me) = staged.view.rows.iter().position(|r| r.local)
        && staged.view.rows[me].rung != rung
    {
        let g = staged_gun(rung);
        let row = &mut staged.bypass_change_detection().view.rows[me];
        row.rung = rung;
        row.gun = g.id;
        row.gun_name = g.name;
        row.class = g.class;
        staged.set_changed();
    }
}

/// The local player's rung ups (a rung up while the race shows).
#[allow(clippy::type_complexity)]
fn watch(
    clock: Res<UiClock>,
    view: Res<RaceView>,
    script: Option<Res<RankScript>>,
    ladder: Option<Res<Ladder>>,
    (names, catalog, games): (
        Option<Res<GunNames>>,
        Option<Res<WeaponCatalog>>,
        Option<Res<chimera_core::game_registry::Games>>,
    ),
    local: Query<&Rung, (With<LocalPlayer>, With<InMinigame>)>,
    mut toast: ResMut<RankToast>,
) {
    if script.is_some() {
        return;
    }
    let (true, Some(ladder), Some(rung)) = (view.active, ladder.as_deref(), local.iter().next()) else {
        if toast.seen.is_some() || !toast.events.is_empty() {
            *toast = RankToast::default();
        }
        return;
    };
    let r = rung.0;
    if let Some(s) = toast.seen
        && r > s
    {
        let gun_of = |rung: usize| {
            ladder
                .shown_weapon(rung, catalog.as_deref())
                .map(|id| toast_gun(id, names.as_deref(), catalog.as_deref(), games.as_deref()))
        };
        let next = if r + 1 < ladder.rungs.len() { gun_of(r + 1) } else { None };
        toast.push(RankEvent { at: clock.now, from: s, to: r, gun: gun_of(r).unwrap_or_default(), next });
    }
    if toast.seen != Some(r) {
        toast.seen = Some(r);
    }
}

/// The toast's root (full width, 6.8 cq down).
#[derive(Component)]
pub struct RankToastRoot;

/// The plate (its fixed width), its fill and the upright clip of its
/// content.
#[derive(Component)]
pub struct ToastPlate;
#[derive(Component)]
pub struct ToastFill;
#[derive(Component)]
pub struct ToastClip;
#[derive(Component)]
pub struct ToastContent;

/// A rolling number (the toast's and the chip's): a clipped box with the
/// old number and the new one (`RollOld`, `RollNew`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Roll {
    /// The box's height in logical px.
    pub h: f32,
    /// The chip's (follows the toast's roll) or the toast's own.
    pub chip: bool,
}
#[derive(Component)]
pub struct RollOld;
#[derive(Component)]
pub struct RollNew;

/// The chip's current tick, popping as the number rolls.
#[derive(Component)]
pub struct TickPop;

/// A gun slot: the new gun (wiping in) or the old (lifting out), and the
/// gun it shows.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GunSlot {
    pub new: bool,
    pub shown: Option<ToastGun>,
}

/// The new gun's wipe (a clip as wide as the wipe).
#[derive(Component)]
pub struct GunWipe;

/// The next gun's box and the gun it shows.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct NextSlot(pub Option<ToastGun>);

/// The toast's sizes in cq (the look book's).
pub const TOAST_W: f32 = 36.4;
const TOAST_H: f32 = 3.1;
const ROLL_H: f32 = 1.6;
const GUN_W: f32 = 16.3;
const GUN_H: f32 = 2.2;
const NAME_W: f32 = 12.4;
const NEXT_W: f32 = 9.6;

fn spawn(look: Look, layer: Res<super::overlay::OverlayLayer>, mut slants: Option<ResMut<Assets<SlantMaterial>>>, mut commands: Commands) {
    let root = commands
        .spawn((
            RankToastRoot,
            ChildOf(layer.0),
            Visibility::Hidden,
            GlobalZIndex(SCOREBOARD_Z - 1),
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                width: percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();
    let plate = commands.spawn((ToastPlate, ChildOf(root), Pickable::IGNORE, Node { flex_shrink: 0.0, ..default() })).id();
    let mut fill = commands.spawn((
        ToastFill,
        ChildOf(plate),
        Pickable::IGNORE,
        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
    ));
    if let Some(slants) = slants.as_deref_mut() {
        fill.insert(MaterialNode(slants.add(SlantMaterial::new(&look.theme, look.tone(Tone::Plate)).edges(false, false).slanted_cut())));
    }
    let clip = commands
        .spawn((
            ToastClip,
            ChildOf(plate),
            Pickable::IGNORE,
            Node { position_type: PositionType::Absolute, top: px(0.0), height: percent(100.0), overflow: Overflow::clip(), ..default() },
        ))
        .id();
    commands.spawn((ToastContent, ChildOf(clip), Pickable::IGNORE, Node { position_type: PositionType::Absolute, ..default() }));
}

/// A rolling number `n + 1` (two digits) `size` cq tall in `tone`, the
/// box clipping it: the chip's (`chip`) or the toast's; the new number's
/// node.
pub fn roll(p: &mut ChildSpawnerCommands, look: &Look, n: usize, size: f32, tone: Tone, chip: bool) -> Entity {
    let h = look.px(size * 1.2);
    let w = look.px(size * 1.4);
    let mut new = Entity::PLACEHOLDER;
    p.spawn((
        Roll { h, chip },
        Node { width: px(w), height: px(h), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
        Pickable::IGNORE,
    ))
    .with_children(|r| {
        let text = format!("{:02}", n + 1);
        let at = |y: f32| Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), ..default() }
            .with_y(y);
        r.spawn((RollOld, at(-1.1 * h), UiTransform::default(), UiOpacity(1.0), look.text(Role::DisplayBold, size, look.tone(tone), &text), Pickable::IGNORE));
        new = r.spawn((RollNew, at(0.0), UiTransform::default(), look.text(Role::DisplayBold, size, look.tone(tone), &text), Pickable::IGNORE)).id();
    });
    new
}

trait WithY {
    fn with_y(self, y: f32) -> Self;
}

impl WithY for Node {
    fn with_y(mut self, y: f32) -> Self {
        self.top = px(y);
        self
    }
}

/// The toast's content (once per layout: the window's size, the theme).
#[allow(clippy::too_many_arguments)]
fn build(
    look: Look,
    mut done: Local<Option<(u32, u32)>>,
    mut plate: Single<&mut Node, With<ToastPlate>>,
    content: Single<Entity, With<ToastContent>>,
    mut root: Single<&mut Node, (With<RankToastRoot>, Without<ToastPlate>)>,
    mut commands: Commands,
) {
    // Nothing to lay out without the Signal look (no toast).
    if !look.on() {
        return;
    }
    let key = (look.units.height as u32, (look.units.scale_factor * 100.0) as u32);
    if *done == Some(key) && !look.theme.is_changed() {
        return;
    }
    *done = Some(key);
    root.top = look.val(look.layout("toast", "top", 6.8));
    plate.width = look.val(TOAST_W);
    plate.height = look.val(look.layout("toast", "h", TOAST_H));
    let content = *content;
    commands.entity(content).despawn_children();
    commands.entity(content).insert(Node {
        position_type: PositionType::Absolute,
        top: px(0.0),
        width: look.val(TOAST_W),
        height: percent(100.0),
        align_items: AlignItems::Center,
        column_gap: look.val(0.9),
        padding: UiRect::new(look.val(1.1), look.val(1.2), px(0.0), px(0.0)),
        ..default()
    });
    commands.entity(content).with_children(|c| {
        // The accent rule along the foot.
        c.spawn((
            Node { position_type: PositionType::Absolute, left: px(0.0), right: px(0.0), bottom: px(0.0), height: look.val(0.14), ..default() },
            BackgroundColor(look.tone(Tone::Acc)),
            Pickable::IGNORE,
        ));
        signal::tag(c, &look, TagKind::Accent, "Rung", 0.62);
        let _ = roll(c, &look, 0, ROLL_H, Tone::Fg, false);
        c.spawn((Node { width: px(look.hair()), height: look.val(1.7), flex_shrink: 0.0, ..default() }, BackgroundColor(look.tone(Tone::Fg4)), Pickable::IGNORE));
        // The guns: the old one (lifting out on a chain) under the new one
        // (wiping in).
        c.spawn((Node { width: look.val(GUN_W), height: look.val(GUN_H), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() }, Pickable::IGNORE))
            .with_children(|g| {
                let full = Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: look.val(GUN_W), height: percent(100.0), ..default() };
                g.spawn((GunSlot { new: false, shown: None }, full.clone(), UiTransform::default(), UiOpacity(0.0), Pickable::IGNORE));
                g.spawn((GunWipe, Node { overflow: Overflow::clip(), width: px(0.0), ..full.clone() }, Pickable::IGNORE)).with_children(|w| {
                    w.spawn((GunSlot { new: true, shown: None }, full.clone(), Pickable::IGNORE));
                });
            });
        // The next gun, set apart by a hairline.
        c.spawn((
            Node {
                width: look.val(NEXT_W),
                height: look.val(1.7),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: look.val(0.45),
                padding: UiRect::left(look.val(0.9)),
                border: UiRect::left(px(look.hair())),
                ..default()
            },
            BorderColor::all(look.tone(Tone::Fg4)),
            Pickable::IGNORE,
        ))
        .with_children(|n| {
            n.spawn((look.text(Role::Mono, 0.48, look.tone(Tone::Fg3), "NEXT"), Pickable::IGNORE));
            n.spawn((
                NextSlot(None),
                UiOpacity(0.0),
                Node { align_items: AlignItems::Center, column_gap: look.val(0.45), flex_shrink: 0.0, ..default() },
                Pickable::IGNORE,
            ));
        });
    });
}

/// A gun slot's content: the icon (centred in 3.2 cq) and the two lines.
fn fill_gun(c: &mut ChildSpawnerCommands, look: &Look, g: &ToastGun, fonts: Option<&super::fonts::UiFonts>, rules: Option<&chimera_sim::weapon::names::GunNameRules>) {
    c.spawn((Node { align_items: AlignItems::Center, column_gap: look.val(0.7), height: percent(100.0), ..default() }, Pickable::IGNORE))
        .with_children(|row| {
            row.spawn((Node { width: look.val(3.2), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, Pickable::IGNORE))
                .with_children(|ib| {
                    if let Some(id) = g.id {
                        signal::gun_icon(ib, look, id, g.class, look.px(1.05), look.tone(Tone::Fg));
                    }
                });
            row.spawn((
                Node { flex_direction: FlexDirection::Column, row_gap: look.val(0.3), width: look.val(NAME_W), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
                Pickable::IGNORE,
            ))
            .with_children(|n| {
                let font = look.font(Role::DisplayBold, 0.94);
                let name = super::overlay::fit(fonts, rules, &font, &g.name.to_uppercase(), look.px(NAME_W));
                n.spawn((look.text(Role::DisplayBold, 0.94, look.tone(Tone::Fg), &name), Pickable::IGNORE));
                n.spawn((look.text(Role::Mono, 0.48, look.tone(Tone::Fg3), &g.line2.to_uppercase()), Pickable::IGNORE));
            });
        });
}

/// Every moving part of the toast and the chip from the pose at the
/// clock's time.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn animate(
    look: Look,
    clock: Res<UiClock>,
    view: Res<RaceView>,
    mut toast: ResMut<RankToast>,
    (fonts, rules): (Option<Res<super::fonts::UiFonts>>, Option<Res<chimera_sim::weapon::names::GunNameRules>>),
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    (mut root, plate, fill, mut clip, mut content): (
        Single<&mut Visibility, With<RankToastRoot>>,
        Single<&ComputedNode, With<ToastPlate>>,
        Single<Option<&MaterialNode<SlantMaterial>>, With<ToastFill>>,
        Single<&mut Node, (With<ToastClip>, Without<ToastContent>)>,
        Single<&mut Node, (With<ToastContent>, Without<ToastClip>)>,
    ),
    rolls: Query<(&Roll, &Children)>,
    mut digits: Query<(&mut UiTransform, &mut Text, Option<&mut UiOpacity>, Has<RollOld>), Or<(With<RollOld>, With<RollNew>)>>,
    mut pops: Query<(&mut Visibility, &mut UiTransform), (With<TickPop>, Without<RankToastRoot>, Without<RollOld>, Without<RollNew>, Without<GunSlot>)>,
    mut slots: Query<(Entity, &mut GunSlot, Option<&mut UiTransform>, Option<&mut UiOpacity>), (Without<RollOld>, Without<RollNew>, Without<TickPop>)>,
    mut wipe: Query<&mut Node, (With<GunWipe>, Without<ToastClip>, Without<ToastContent>)>,
    mut next: Query<(Entity, &mut NextSlot, &mut UiOpacity), (Without<GunSlot>, Without<RollOld>, Without<RollNew>)>,
    mut commands: Commands,
) {
    let d = ToastTimes::of(&look);
    let events: Vec<RankEvent> = toast.events.iter().cloned().collect();
    let pose = if view.active { pose(events.iter().collect(), clock.now, &d) } else { None };
    // Old rung ups gone by: keep the last (a chain looks back one).
    if pose.is_none() && toast.events.len() > 1 && events.last().is_some_and(|e| e.at <= clock.now) {
        let last = toast.events.pop_back();
        toast.events.clear();
        toast.events.extend(last);
    }
    root.set_if_neq(if pose.as_ref().is_some_and(ToastPose::visible) { Visibility::Inherited } else { Visibility::Hidden });

    // The rolling numbers: the toast's always while it shows, the chip's
    // only while it rolls.
    for (roll, children) in &rolls {
        let shown = pose.as_ref().filter(|p| !roll.chip || p.roll < 1.0);
        for c in children.iter() {
            let Ok((mut tf, mut text, opacity, old)) = digits.get_mut(c) else { continue };
            let (y, a, n) = match shown {
                Some(p) => {
                    let e = &events[p.k];
                    if old { (p.old_y, p.old_alpha, e.from) } else { (p.new_y, 1.0, e.to) }
                }
                None if old => (-1.1, 0.0, 0),
                None => (0.0, 1.0, usize::MAX),
            };
            // Placed from the box's top: the old number sits at -1.1.
            let base = if old { -1.1 } else { 0.0 };
            let ty = px((y - base) * roll.h);
            if tf.translation.y != ty {
                tf.translation.y = ty;
            }
            if n != usize::MAX {
                let s = format!("{:02}", n + 1);
                if text.0 != s {
                    text.0 = s;
                }
            }
            if let Some(mut o) = opacity {
                o.set_if_neq(UiOpacity(a));
            }
        }
    }
    for (mut vis, mut tf) in &mut pops {
        match pose.as_ref().filter(|p| p.rolling()) {
            Some(p) => {
                vis.set_if_neq(Visibility::Inherited);
                if tf.scale.y != p.pop {
                    tf.scale.y = p.pop;
                }
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
    let Some(p) = pose else { return };

    // The wipe in and out: the fill's cut along the slant, the content
    // clipped upright (and a lean's width wider, so the cut shows).
    let w = plate.size.x * plate.inverse_scale_factor;
    let lean = look.px(look.layout("toast", "h", TOAST_H)) * look.theme.tan();
    let a = (p.exit * w).round();
    let b = (p.enter * w + lean).round().min(w);
    if clip.left != px(a) || clip.width != px((b - a).max(0.0)) {
        clip.left = px(a);
        clip.width = px((b - a).max(0.0));
    }
    if content.left != px(-a) {
        content.left = px(-a);
    }
    if let (Some(m), Some(slants)) = (*fill, slants.as_deref_mut())
        && let Some(mat) = slants.get(&m.0)
        && (mat.wipe.x != p.exit || mat.wipe.y != 1.0 - p.enter)
        && let Some(mut mat) = slants.get_mut(&m.0)
    {
        mat.set_clip(p.exit, 1.0 - p.enter);
    }

    // The guns.
    let e = &events[p.k];
    let gun_h = look.px(GUN_H);
    for (ent, mut slot, tf, opacity) in &mut slots {
        let want = if slot.new {
            Some(e.gun.clone())
        } else {
            p.old_gun.map(|_| events[p.k - 1].gun.clone())
        };
        let rebuilt = slot.shown != want;
        if rebuilt {
            slot.shown = want.clone();
            commands.entity(ent).despawn_children();
            if let Some(g) = &want {
                commands.entity(ent).with_children(|c| fill_gun(c, &look, g, fonts.as_deref(), rules.as_deref()));
            }
        }
        if !slot.new {
            let (y, a) = p.old_gun.unwrap_or((0.0, 0.0));
            if let Some(mut tf) = tf {
                let ty = px(y * gun_h);
                if tf.translation.y != ty {
                    tf.translation.y = ty;
                }
            }
            if let Some(mut o) = opacity {
                if rebuilt {
                    o.set_changed();
                }
                o.set_if_neq(UiOpacity(a));
            }
        }
    }
    for mut n in &mut wipe {
        let v = px((p.gun * look.px(GUN_W)).round());
        if n.width != v {
            n.width = v;
        }
    }
    // The next gun.
    for (ent, mut slot, mut o) in &mut next {
        let want = events[p.next_of].next.clone();
        if slot.0 != want {
            slot.0 = want.clone();
            commands.entity(ent).despawn_children();
            if let Some(g) = &want {
                commands.entity(ent).with_children(|n| {
                    if let Some(id) = g.id {
                        signal::gun_icon(n, &look, id, g.class, look.px(0.66), look.tone(Tone::Fg2));
                    }
                    n.spawn((look.text(Role::DisplayBold, 0.58, look.tone(Tone::Fg2), &g.name.to_uppercase()), Pickable::IGNORE));
                });
            } else {
                commands.entity(ent).with_children(|n| {
                    n.spawn((look.text(Role::DisplayBold, 0.58, look.tone(Tone::AccText), "FINAL"), Pickable::IGNORE));
                });
            }
            o.set_changed();
        }
        o.set_if_neq(UiOpacity(p.next_alpha));
    }
}

/// The chip's meter's tick pop over the current tick `x` logical px from
/// the meter's left, `w` by `h` (the race chip's builder calls it).
pub fn tick_pop(p: &mut ChildSpawnerCommands, look: &Look, slants: Option<&mut Assets<SlantMaterial>>, x: f32, w: f32, h: f32) {
    let mut e = p.spawn((
        TickPop,
        Visibility::Hidden,
        UiTransform::default(),
        Node { position_type: PositionType::Absolute, left: px(x), top: px(0.0), width: px(w), height: px(h), ..default() },
        Pickable::IGNORE,
    ));
    match slants {
        Some(s) => {
            e.insert(MaterialNode(s.add(SlantMaterial::new(&look.theme, look.tone(Tone::Acc)))));
        }
        None => {
            e.insert(BackgroundColor(look.tone(Tone::Acc)));
        }
    }
}
