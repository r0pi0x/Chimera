//! The bench (T in the sandbox; the design rules, sections 20 and 21,
//! `sb2-bench`, `sb2-bench-pick`): a drawer at the right, the HUD's chip
//! staying where it is.
//!
//! - **Targets** (up to `link::MAX_TARGETS`; ADD TARGET, N): a card each
//!   with the character's portrait, name and game, its distance and
//!   motion, and a trash can. The selected card opens: Character (opens
//!   the character picker), Holds (its gun; its default unless chosen),
//!   Protection (its shield, gear or armour on, or health only: the
//!   mechanics' `armored`), Never falls (hits measured, it stays up:
//!   `protected`), Place (Line, or Platform: the range's raised
//!   platform, where it stands still, so Strafes and Walks are greyed
//!   out and no distance is picked; a distance puts it back on that
//!   line; off the range the mechanics refuse it and the reason shows),
//!   Distance (5 to 200 m),
//!   Moves (Still, Strafes, Walks), Shoots back (its own toggle, off
//!   unless you want it). Every change is a `link::BenchRequest` at once.
//!   A refused spawn's reason shows under the list.
//! - **Props** (for accuracy; ADD PROP cycles the kinds): bottles, tin
//!   cans, a target board, each at its distance, how many pieces still
//!   stand or the board's group (hits, spread, mean radius), Reset (or
//!   Clear: the group) and a trash can.
//! - **Options**: the sandbox's toggles (`link::SandboxOptions`), each the
//!   mechanics' condition (`conditions::SetCondition`: God, EndlessAmmo,
//!   TimeScale 0.5, ThirdPerson; blast rings are the screens' own), shown
//!   as the mechanics hold them (`sandbox::mirror_conditions`).
//! - **Foot**: "Changes apply at once." and RESET BENCH (clears targets
//!   and props).
//! - **The character picker** (a dialog over the range): games at the
//!   left, the characters by game as portrait tiles in the middle, the
//!   chosen one at the right (its protection, what it holds); a press
//!   selects, a double press or USE picks it (the kit's double-click
//!   rule), Esc or CANCEL closes. Characters come from
//!   `CharacterCatalogue` (the mechanics' or the asset side's list of
//!   bodies; staged by hand until one exists).
//!
//! - **Holds** opens a list under it: Its default, then every gun the
//!   target's body may hold (`holds_of`: `CharacterEntry::guns`, the
//!   mechanics' `CharacterInfo::guns`); a press holds it (a
//!   `BenchRequest::Set` with the gun), Esc closes the list.
//! - The picker's "Find a character or a game" takes what is typed
//!   while the picker is open (`BenchUi::found`: every word in a
//!   character's name, game or id); Esc clears it, then closes the
//!   picker.
//!
//! Keys: ↑↓ the target, N adds one, Delete removes the selected one,
//! Esc (or T) closes.
//!
//! Known shortcuts: portraits are the kit's profile mark (no generated
//! per-asset portraits yet, design rules section 21); the drawer is
//! rebuilt whole on a change.

use bevy::{prelude::*, ui::{DeprecatedInteraction as Interaction, FocusPolicy}};
use chimera_ui::{
    Role, Tone, UiParityId, UiStyle,
    controls::{HERO_H, Kind, State, Ui, clickable},
};

use super::{
    SandboxScreen, SandboxUi,
    hud::{M, kit_icon, tag},
    link::{BenchRequest, BenchView, DISTANCES, MAX_TARGETS, PROPS, PropSpec, SandboxOption, SandboxOptions, TargetMotion, TargetPlace, TargetSpec, place_label},
};
use crate::client::signal::Look;

/// A body a target may be.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterEntry {
    pub id: String,
    pub name: String,
    pub game: String,
    /// Shield and health.
    pub shield: f32,
    pub health: f32,
    /// What it holds by default (the first of `guns`), and the guns it
    /// may hold (its game's: the body follows the held gun's game).
    pub default_gun: String,
    pub guns: Vec<String>,
    /// Its files' download size (None: here).
    pub bytes: Option<u64>,
}

/// The bodies (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CharacterCatalogue(pub Vec<CharacterEntry>);

/// The bench's own state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BenchUi {
    /// The open target card (an index into `BenchView::targets`).
    pub selected: usize,
    /// The character picker, open for a target (None: a new one), and
    /// the character chosen in it.
    pub picker: Option<(Option<u64>, Option<String>)>,
    /// The next prop ADD PROP adds.
    pub next_prop: usize,
    /// The open card's Holds list is open.
    pub holds_open: bool,
    /// What is typed in the picker's "Find a character or a game".
    pub picker_search: String,
    /// Its caret and the other end of the selection (char indices).
    pub picker_caret: usize,
    pub picker_anchor: usize,
}

impl BenchUi {
    /// Empties the picker's search field.
    pub fn clear_search(&mut self) {
        self.picker_search.clear();
        self.picker_caret = 0;
        self.picker_anchor = 0;
    }

    /// The picker's characters that hold every typed word in their name,
    /// game or id (all of them with nothing typed).
    pub fn found<'a>(&self, chars: &'a [CharacterEntry]) -> Vec<&'a CharacterEntry> {
        let words: Vec<String> = self.picker_search.split_whitespace().map(str::to_lowercase).collect();
        chars
            .iter()
            .filter(|c| {
                let hay = format!("{} {} {}", c.name, c.game, c.id).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }
}

/// The guns a target of `character` may hold (`CharacterEntry::guns`,
/// the mechanics' `CharacterInfo::guns`; none when the body is unknown).
pub fn holds_of<'a>(chars: &'a [CharacterEntry], character: &str) -> &'a [String] {
    chars.iter().find(|c| c.id == character).map_or(&[], |c| c.guns.as_slice())
}

/// A click on the bench.
#[derive(Component, Message, Clone, Debug, PartialEq)]
pub enum BenchAction {
    Select(usize),
    Add,
    Remove(u64),
    Distance(u64, f32),
    Motion(u64, TargetMotion),
    /// Onto the range's platform (standing still) or back on a line.
    Place(u64, TargetPlace),
    Protected(u64, bool),
    Armored(u64, bool),
    ShootsBack(u64, bool),
    Character(Option<u64>),
    /// Open or close the open card's Holds list.
    HoldsOpen,
    /// What the target holds (None: its default).
    Hold(u64, Option<String>),
    PickerChoose(String),
    PickerUse,
    PickerCancel,
    AddProp,
    RemoveProp(u64),
    ResetProp(u64),
    Option(SandboxOption, bool),
    Reset,
    Close,
}

#[derive(Component)]
struct BenchRoot;

/// The picker's own root: it is rebuilt only when the picker's inputs
/// change (its list binds its slots a frame after it is built, so a
/// rebuild for an unrelated change of the bench would leave it empty).
#[derive(Component)]
struct PickerRoot;

/// What the picker shows.
#[derive(Clone, Debug, PartialEq)]
struct PickShown {
    picker: (Option<u64>, Option<String>),
    search: String,
    caret: (usize, usize),
    chars: usize,
    /// The target's number in the title.
    n: usize,
    size: (u32, u32),
}

/// The drawer's width, cq.
pub const W: f32 = 31.0;

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    console: Option<Res<crate::client::console::ConsoleUi>>,
    ui: Res<SandboxUi>,
    mut bench: ResMut<BenchUi>,
    view: Res<BenchView>,
    mut out: MessageWriter<BenchAction>,
    mut own: Local<chimera_ui::clipboard::Clipboard>,
    shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    if ui.screen != SandboxScreen::Bench || console.is_some_and(|c| c.open) {
        typed.clear();
        return;
    }
    let k = |c: KeyCode| keys.just_pressed(c);
    if bench.picker.is_some() {
        // Typing finds a character ("Find a character or a game"): the
        // kit's text field, with its caret, selection, copy and paste.
        use bevy::input::ButtonState;
        let held = |ks: [KeyCode; 2]| keys.any_pressed(ks);
        let (ctrl, shift) = (held([KeyCode::ControlLeft, KeyCode::ControlRight]), held([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
        let clipboard = match shared {
            Some(c) => c.into_inner(),
            None => &mut *own,
        };
        for e in typed.read() {
            if e.state != ButtonState::Pressed {
                continue;
            }
            let Some(key) = crate::client::widgets::text_key(&e.logical_key, e.key_code, ctrl, shift) else { continue };
            let mut caret = crate::client::widgets::Caret { at: bench.picker_caret, anchor: bench.picker_anchor };
            let mut text = bench.picker_search.clone();
            if caret.key(&mut text, key, clipboard, 32, false) {
                bench.picker_search = text;
            }
            let n = bench.picker_search.chars().count();
            let (at, anchor) = (caret.at.min(n), caret.anchor.min(n));
            if (bench.picker_caret, bench.picker_anchor) != (at, anchor) {
                bench.picker_caret = at;
                bench.picker_anchor = anchor;
            }
        }
        if k(KeyCode::Enter) {
            out.write(BenchAction::PickerUse);
        }
        return;
    }
    typed.clear();
    if k(KeyCode::KeyN) {
        out.write(BenchAction::Add);
    }
    if k(KeyCode::ArrowDown) && bench.selected + 1 < view.targets.len() {
        out.write(BenchAction::Select(bench.selected + 1));
    }
    if k(KeyCode::ArrowUp) && bench.selected > 0 {
        out.write(BenchAction::Select(bench.selected - 1));
    }
    if k(KeyCode::Delete)
        && let Some(t) = view.targets.get(bench.selected)
    {
        out.write(BenchAction::Remove(t.id));
    }
}

fn clicks(pressed: Query<(&Interaction, &BenchAction), Changed<Interaction>>, clock: Option<Res<chimera_ui::UiClock>>, mut last: Local<Option<(String, f64)>>, mut out: MessageWriter<BenchAction>) {
    let now = clock.map_or(0.0, |c| c.now);
    for (i, a) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        // The kit's double-click rule: a second press on a character uses it.
        if let BenchAction::PickerChoose(id) = a {
            let double = last.as_ref().is_some_and(|(l, t)| l == id && now - t <= chimera_ui::controls::DOUBLE_CLICK_SECS);
            *last = Some((id.clone(), now));
            out.write(a.clone());
            if double {
                out.write(BenchAction::PickerUse);
            }
            continue;
        }
        out.write(a.clone());
    }
}

fn act(world: &mut World) {
    let actions: Vec<BenchAction> = world.resource_mut::<Messages<BenchAction>>().drain().collect();
    for a in actions {
        let view = world.resource::<BenchView>().clone();
        let spec_of = |id: u64| view.targets.iter().find(|t| t.id == id).map(|t| t.spec.clone());
        let req = |w: &mut World, r: BenchRequest| {
            w.write_message(r);
        };
        match a {
            BenchAction::Select(i) => {
                let mut b = world.resource_mut::<BenchUi>();
                b.selected = i;
                b.holds_open = false;
            }
            BenchAction::HoldsOpen => {
                let mut b = world.resource_mut::<BenchUi>();
                b.holds_open = !b.holds_open;
            }
            BenchAction::Hold(id, gun) => {
                world.resource_mut::<BenchUi>().holds_open = false;
                if let Some(s) = spec_of(id)
                    && s.gun != gun
                {
                    req(world, BenchRequest::Set(id, TargetSpec { gun, ..s }));
                }
            }
            BenchAction::Add => {
                if view.targets.len() < MAX_TARGETS {
                    world.resource_mut::<BenchUi>().picker = Some((None, None));
                }
            }
            BenchAction::Remove(id) => req(world, BenchRequest::Remove(id)),
            BenchAction::Distance(id, d) => {
                // A distance is a line's: a platform target goes back on it.
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { distance_m: d, place: TargetPlace::Line, ..s }));
                }
            }
            BenchAction::Motion(id, m) => {
                // On the platform it stands still.
                if let Some(s) = spec_of(id)
                    && (s.place == TargetPlace::Line || m == TargetMotion::Still)
                {
                    req(world, BenchRequest::Set(id, TargetSpec { motion: m, ..s }));
                }
            }
            BenchAction::Place(id, p) => {
                if let Some(s) = spec_of(id)
                    && s.place != p
                {
                    let next = match p {
                        TargetPlace::Platform => TargetSpec { place: p, motion: TargetMotion::Still, ..s },
                        TargetPlace::Line => TargetSpec { place: p, distance_m: TargetSpec::default().distance_m, ..s },
                    };
                    req(world, BenchRequest::Set(id, next));
                }
            }
            BenchAction::Protected(id, on) => {
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { protected: on, ..s }));
                }
            }
            BenchAction::Armored(id, on) => {
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { armored: on, ..s }));
                }
            }
            BenchAction::ShootsBack(id, on) => {
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { shoots_back: on, ..s }));
                }
            }
            BenchAction::Character(id) => {
                let cur = id.and_then(spec_of).map(|s| s.character);
                world.resource_mut::<BenchUi>().picker = Some((id, cur));
            }
            BenchAction::PickerChoose(c) => {
                if let Some(p) = world.resource_mut::<BenchUi>().picker.as_mut() {
                    p.1 = Some(c);
                }
            }
            BenchAction::PickerUse => {
                let Some((target, Some(ch))) = world.resource_mut::<BenchUi>().picker.take() else { continue };
                world.resource_mut::<BenchUi>().clear_search();
                match target.and_then(|id| spec_of(id).map(|s| (id, s))) {
                    Some((id, s)) => req(world, BenchRequest::Set(id, TargetSpec { character: ch, gun: None, ..s })),
                    None => req(world, BenchRequest::Spawn(TargetSpec { character: ch, ..default() })),
                }
            }
            BenchAction::PickerCancel => {
                let mut b = world.resource_mut::<BenchUi>();
                b.picker = None;
                b.clear_search();
            }
            BenchAction::AddProp => {
                let mut b = world.resource_mut::<BenchUi>();
                let (kind, ..) = PROPS[b.next_prop % PROPS.len()];
                b.next_prop += 1;
                let d = [20.0, 10.0, 25.0][(b.next_prop - 1) % 3];
                req(world, BenchRequest::AddProp(PropSpec { kind: kind.into(), distance_m: d }));
            }
            BenchAction::RemoveProp(id) => req(world, BenchRequest::RemoveProp(id)),
            BenchAction::ResetProp(id) => req(world, BenchRequest::ResetProp(id)),
            BenchAction::Option(o, on) => {
                let staged = world.resource::<SandboxUi>().staged;
                let was = world.resource::<SandboxOptions>().get(o);
                if was != on {
                    world.resource_mut::<SandboxOptions>().set(o, on);
                    // The mechanics' condition (`conditions::SetCondition`);
                    // blast rings are the screens' own.
                    use chimera_sim::minigame::conditions::{Condition, SetCondition};
                    let cond = match o {
                        SandboxOption::EndlessAmmo => Some(Condition::EndlessAmmo(on)),
                        SandboxOption::NoDamage => Some(Condition::God(on)),
                        SandboxOption::HalfSpeed => Some(Condition::TimeScale(if on { 0.5 } else { 1.0 })),
                        SandboxOption::ThirdPerson => Some(Condition::ThirdPerson(on)),
                        SandboxOption::BlastRings => None,
                    };
                    if !staged
                        && let Some(cond) = cond
                        && world.contains_resource::<Messages<SetCondition>>()
                    {
                        world.write_message(SetCondition(cond));
                    }
                }
            }
            BenchAction::Reset => req(world, BenchRequest::Clear),
            BenchAction::Close => {
                world.resource_mut::<SandboxUi>().screen = SandboxScreen::None;
            }
        }
    }
    // Esc: the picker (its search first), the Holds list first.
    let (picker_open, holds_open) = {
        let b = world.resource::<BenchUi>();
        (b.picker.is_some(), b.holds_open)
    };
    let esc = world.get_resource::<ButtonInput<KeyCode>>().is_some_and(|k| k.just_pressed(KeyCode::Escape));
    // (The sandbox's keys ran first and closed the bench: a picker or a
    // list is only ever open on it.)
    if (picker_open || holds_open) && esc && world.resource::<SandboxUi>().on {
        let mut b = world.resource_mut::<BenchUi>();
        if !b.picker_search.is_empty() {
            b.clear_search();
        } else if picker_open {
            b.picker = None;
        } else {
            b.holds_open = false;
        }
        // The screen stays: undo the close the sandbox's keys asked for.
        world.resource_mut::<SandboxUi>().screen = SandboxScreen::Bench;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Shown {
    open: bool,
    view: BenchView,
    bench: BenchUi,
    options: SandboxOptions,
    chars: usize,
    guns: usize,
    size: (u32, u32),
}

#[allow(clippy::too_many_arguments)]
fn draw(
    look: Look,
    ui_state: Res<SandboxUi>,
    view: Res<BenchView>,
    bench: Res<BenchUi>,
    options: Res<SandboxOptions>,
    chars: Res<CharacterCatalogue>,
    cat: Res<super::SandboxCatalogue>,
    root: Query<Entity, With<BenchRoot>>,
    mut lists: ResMut<chimera_ui::VirtualLists>,
    picker_root: Query<Entity, With<PickerRoot>>,
    mut last_pick: Local<Option<PickShown>>,
    mut last: Local<Shown>,
    mut c: Commands,
) {
    let open = look.on() && ui_state.on && ui_state.screen == SandboxScreen::Bench;
    let pick_now = open
        .then(|| {
            bench.picker.clone().map(|p| PickShown {
                n: p.0.and_then(|id| view.targets.iter().position(|t| t.id == id)).unwrap_or(view.targets.len()) + 1,
                picker: p,
                search: bench.picker_search.clone(),
                caret: (bench.picker_caret, bench.picker_anchor),
                chars: chars.0.len(),
                size: (look.units.height as u32, (look.units.scale_factor * 100.0) as u32),
            })
        })
        .flatten();
    if *last_pick != pick_now {
        *last_pick = pick_now.clone();
        for r in &picker_root {
            c.entity(r).despawn();
        }
        if let Some(p) = pick_now {
            let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
            let full = c
                .spawn((
                    PickerRoot,
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    GlobalZIndex(super::change_gun::Z + 1),
                    Pickable::IGNORE,
                ))
                .id();
            picker(&mut c, &ui, &look, full, &chars.0, p.picker.0, p.picker.1.as_deref(), &view, &bench, &mut lists);
        }
    }
    let next = Shown {
        open: look.on() && ui_state.on && ui_state.screen == SandboxScreen::Bench,
        view: view.clone(),
        // The picker has its own root (`PickerRoot`).
        bench: BenchUi { picker: None, picker_search: String::new(), picker_caret: 0, picker_anchor: 0, ..bench.clone() },
        options: *options,
        chars: chars.0.len(),
        guns: cat.guns.len(),
        size: (look.units.height as u32, (look.units.scale_factor * 100.0) as u32),
    };
    if *last == next {
        return;
    }
    *last = next.clone();
    for r in &root {
        c.entity(r).despawn();
    }
    if !next.open {
        return;
    }
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    let full = c
        .spawn((
            BenchRoot,
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            GlobalZIndex(super::change_gun::Z),
            Pickable::IGNORE,
        ))
        .id();
    let name = |id: &str| cat.get(id).map_or_else(|| id.to_string(), |g| g.name.clone());
    drawer(&mut c, &ui, &look, full, &view, &bench, &options, &chars.0, &name);
}

fn button(c: &mut Commands, ui: &Ui, parent: Entity, label: &str, key: Option<&str>, a: BenchAction) -> Entity {
    let b = ui.button(c, parent, Kind::Secondary, label, key, 1.9, State::NONE);
    c.entity(b).insert(a);
    b
}

fn trash(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, a: BenchAction) {
    let b = c
        .spawn((
            UiStyle { w: Some(1.5), h: Some(1.5), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), margin: Some([0.0, 0.0, 0.0, 0.3].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0),
            Node::default(),
            a,
            clickable(),
            ChildOf(parent),
        ))
        .id();
    kit_icon(c, look, b, "trash", 0.62, Tone::Fg2, 1.0);
    let _ = ui;
}

fn portrait(c: &mut Commands, ui: &Ui, look: &Look, parent: Entity, s: f32) {
    let p = ui.boxed(c, parent, UiStyle { w: Some(s), h: Some(s), justify: Some(JustifyContent::Center), align: Some(AlignItems::FlexEnd), shrink: Some(0.0), ..UiStyle::row() }.bg(Tone::Ink3).outline(Tone::Fg5, 1.0));
    kit_icon(c, look, p, "profile", s * 0.8, Tone::Fg3, 1.0);
}

fn section(c: &mut Commands, ui: &Ui, parent: Entity, title: &str, note: &str) -> Entity {
    let r = ui.boxed(c, parent, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), margin: Some([0.2, 0.0, 0.1, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, r, Tone::Acc, 0.3, 0.7);
    ui.text(c, r, Role::Mono, 0.5, Tone::Fg2, title);
    ui.text(c, r, Role::Mono, 0.46, Tone::Fg3, note);
    ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    r
}

fn small_toggle(c: &mut Commands, ui: &Ui, parent: Entity, on: bool, a: BenchAction) {
    let t = ui.toggle(c, parent, on, State::NONE);
    c.entity(t).insert((a, clickable()));
}

#[allow(clippy::too_many_arguments)]
fn drawer(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, view: &BenchView, bench: &BenchUi, options: &SandboxOptions, chars: &[CharacterEntry], name: &dyn Fn(&str) -> String) {
    let d = c
        .spawn((
            UiStyle { w: Some(W), gap: Some(0.35), pad: Some([1.0, 1.4, 0.0, 1.4].into()), ..UiStyle::column() }.abs([None, Some(1.6), Some(M), Some(1.6)]).bg(Tone::Ink2).outline(Tone::Fg5, 1.0),
            Node::default(),
            FocusPolicy::Block,
            Interaction::default(),
            UiParityId::new("bench.drawer"),
            ChildOf(root),
        ))
        .id();
    let back = ui.boxed(c, d, UiStyle { gap: Some(0.45), align: Some(AlignItems::Center), ..UiStyle::row() });
    kit_icon(c, look, back, "back", 0.5, Tone::Fg2, 1.0);
    ui.text(c, back, Role::Mono, 0.52, Tone::Fg2, "Sandbox");
    ui.kbd(c, back, "T", 0.5, Tone::Fg3);
    ui.kbd(c, back, "Esc", 0.5, Tone::Fg3);
    ui.text(c, d, Role::DisplayBold, 1.5, Tone::Fg, "Bench");
    // Targets.
    let s = section(c, ui, d, "Targets", &format!("{} of {MAX_TARGETS}", view.targets.len()));
    button(c, ui, s, "Add target", Some("N"), BenchAction::Add);
    for (i, t) in view.targets.iter().enumerate() {
        let open = i == bench.selected.min(view.targets.len().saturating_sub(1));
        let card = c
            .spawn((
                UiStyle { gap: Some(0.1), pad: Some([0.25, 0.6, 0.25, 0.8].into()), margin: Some([0.0, 0.0, 0.15, 0.0].into()), ..UiStyle::column() }.bg_a(Tone::Fg, if open { 0.06 } else { 0.02 }).outline(Tone::Fg5, 1.0),
                Node::default(),
                UiParityId::new(format!("bench.target.{}", i + 1)),
                ChildOf(d),
            ))
            .id();
        if open {
            ui.focus_bar(c, card, 0.24);
        }
        let head = c
            .spawn((UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), h: Some(1.95), ..UiStyle::row() }, Node::default(), BenchAction::Select(i), clickable(), ChildOf(card)))
            .id();
        portrait(c, ui, look, head, 1.7);
        // The name and the stats give way (cut with an ellipsis), the
        // trash never does.
        let tx = ui.boxed(c, head, UiStyle { gap: Some(0.25), min_w: Some(0.0), grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
        let nm = ui.text(c, tx, Role::DisplayRegular, 0.74, Tone::Fg, &t.name);
        c.entity(nm).insert(chimera_ui::Ellipsize::in_parent(t.name.to_uppercase()));
        let gm = ui.text(c, tx, Role::Mono, 0.42, Tone::Fg3, &t.game);
        c.entity(gm).insert(chimera_ui::Ellipsize::in_parent(t.game.to_uppercase()));
        let sum = match t.respawn {
            Some(_) => "Down \u{00b7} back soon".to_string(),
            None if t.spec.place == TargetPlace::Platform => format!("Platform \u{00b7} {}", t.spec.motion.label()),
            None => format!("{} m \u{00b7} {}", t.spec.distance_m, t.spec.motion.label()),
        };
        let sb = ui.boxed(c, head, UiStyle { min_w: Some(0.0), shrink: Some(2.0), ..UiStyle::row() });
        let st = ui.text(c, sb, Role::Mono, 0.46, Tone::Fg2, &sum);
        c.entity(st).insert(chimera_ui::Ellipsize::in_parent(sum.to_uppercase()));
        trash(c, ui, look, head, BenchAction::Remove(t.id));
        if !open {
            continue;
        }
        let row = |c: &mut Commands, label: &str| {
            let r = ui.boxed(c, card, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), h: Some(1.55), ..UiStyle::row() });
            let l = ui.boxed(c, r, UiStyle { w: Some(5.0), shrink: Some(0.0), ..UiStyle::row() });
            ui.text(c, l, Role::DisplayRegular, 0.56, Tone::Fg2, label);
            r
        };
        let r = row(c, "Character");
        let f = c
            .spawn((UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), grow: Some(1.0), h: Some(1.45), pad: Some([0.0, 0.5, 0.0, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.04), Node::default(), BenchAction::Character(Some(t.id)), clickable(), ChildOf(r)))
            .id();
        ui.underline(c, f, Tone::Fg3, 0.05);
        ui.text(c, f, Role::DisplayRegular, 0.64, Tone::Fg, &t.name);
        ui.text(c, f, Role::Mono, 0.42, Tone::Fg3, &short_game(&t.game));
        ui.boxed(c, f, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        kit_icon(c, look, f, "listchev", 0.6, Tone::Fg2, 1.0);
        let r = row(c, "Holds");
        let f = c
            .spawn((
                UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), grow: Some(1.0), h: Some(1.45), pad: Some([0.0, 0.5, 0.0, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Fg, if bench.holds_open { 0.1 } else { 0.04 }),
                Node::default(),
                BenchAction::HoldsOpen,
                clickable(),
                UiParityId::new("bench.holds"),
                ChildOf(r),
            ))
            .id();
        ui.underline(c, f, if bench.holds_open { Tone::Acc } else { Tone::Fg3 }, 0.05);
        let held = t.spec.gun.as_deref().map_or_else(|| "Its default".to_string(), name);
        ui.text(c, f, Role::DisplayRegular, 0.64, Tone::Fg, &held);
        ui.boxed(c, f, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        kit_icon(c, look, f, "listchev", 0.6, Tone::Fg2, 1.0);
        if bench.holds_open {
            // The list: its default, then every gun its body may hold
            // (`holds_of`), in the ladder's order, the held one marked.
            let guns = holds_of(chars, &t.spec.character);
            let list = c
                .spawn((
                    UiStyle { gap: Some(0.05), pad: Some([0.2, 0.0, 0.2, 0.0].into()), margin: Some([0.0, 0.0, 0.0, 5.6].into()), ..UiStyle::column() }.bg(Tone::Ink3).outline(Tone::Fg4, 1.0),
                    Node { max_height: ui.units.val(12.0), overflow: Overflow::clip_y(), ..default() },
                    UiParityId::new("bench.holds.list"),
                    ChildOf(card),
                ))
                .id();
            let item = |c: &mut Commands, label: &str, note: &str, on: bool, a: BenchAction| {
                let it = c
                    .spawn((
                        UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), h: Some(1.3), pad: Some([0.0, 0.6, 0.0, 0.6].into()), shrink: Some(0.0), ..UiStyle::row() }.bg_a(Tone::Fg, if on { 0.1 } else { 0.0 }),
                        Node::default(),
                        a,
                        clickable(),
                        ChildOf(list),
                    ))
                    .id();
                if on {
                    ui.focus_bar(c, it, 0.2);
                }
                ui.text(c, it, Role::DisplayRegular, 0.56, if on { Tone::Fg } else { Tone::Fg2 }, label);
                ui.boxed(c, it, UiStyle { grow: Some(1.0), ..UiStyle::row() });
                ui.text(c, it, Role::Mono, 0.38, Tone::Fg3, note);
            };
            item(c, "Its default", "", t.spec.gun.is_none(), BenchAction::Hold(t.id, None));
            for g in guns {
                item(c, &name(g), g.split_once(':').map_or("", |(_, w)| w), t.spec.gun.as_deref() == Some(g.as_str()), BenchAction::Hold(t.id, Some(g.clone())));
            }
            if guns.is_empty() {
                ui.text(c, list, Role::Mono, 0.4, Tone::Fg3, "No other gun for this body");
            }
        }
        let r = row(c, "Protection");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg2, if t.spec.armored { "Shield or gear \u{00b7} health" } else { "Health only" });
        small_toggle(c, ui, r, t.spec.armored, BenchAction::Armored(t.id, !t.spec.armored));
        let r = row(c, "Never falls");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg3, if t.spec.protected { "Hits measured, it stays up" } else { "Falls, back in 3 s" });
        small_toggle(c, ui, r, t.spec.protected, BenchAction::Protected(t.id, !t.spec.protected));
        let on_platform = t.spec.place == TargetPlace::Platform;
        let r = row(c, "Place");
        c.entity(r).insert(UiParityId::new("bench.place"));
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        if on_platform {
            ui.text(c, r, Role::Mono, 0.44, Tone::Fg3, "Stands still up there");
        }
        let places = [TargetPlace::Line, TargetPlace::Platform];
        let names: Vec<&str> = places.iter().map(|p| place_label(*p)).collect();
        for (k, e) in ui.segmented(c, r, &names, places.iter().position(|p| *p == t.spec.place), State::NONE).into_iter().enumerate() {
            c.entity(e).insert((BenchAction::Place(t.id, places[k]), clickable()));
        }
        let r = row(c, "Distance");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let words: Vec<String> = DISTANCES.iter().map(|d| format!("{d} m")).collect();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        // (A platform target has no line: none picked; a press puts it on one.)
        let sel = DISTANCES.iter().position(|d| !on_platform && (*d - t.spec.distance_m).abs() < 0.01);
        for (k, e) in ui.segmented(c, r, &refs, sel, State::NONE).into_iter().enumerate() {
            c.entity(e).insert((BenchAction::Distance(t.id, DISTANCES[k]), clickable()));
        }
        let r = row(c, "Moves");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let names: Vec<&str> = TargetMotion::ALL.iter().map(|m| m.label()).collect();
        let sel = TargetMotion::ALL.iter().position(|m| *m == t.spec.motion);
        // On the platform it stands still: Strafes and Walks greyed out.
        let off: &[usize] = if on_platform { &[1, 2] } else { &[] };
        c.entity(r).insert(UiParityId::new("bench.moves"));
        for (k, e) in ui.segmented_with(c, r, &names, sel, off, State::NONE).into_iter().enumerate() {
            if !off.contains(&k) {
                c.entity(e).insert((BenchAction::Motion(t.id, TargetMotion::ALL[k]), clickable()));
            }
        }
        let r = row(c, "Shoots back");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg3, "Off unless you want it");
        small_toggle(c, ui, r, t.spec.shoots_back, BenchAction::ShootsBack(t.id, !t.spec.shoots_back));
    }
    if let Some(why) = &view.refused {
        let r = ui.boxed(c, d, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, r, Tone::Threat, 0.3, 0.6);
        ui.sentence(c, r, 0.62, Tone::Fg, why);
    }
    // Props.
    let s = section(c, ui, d, "Props", "For accuracy");
    button(c, ui, s, "Add prop", None, BenchAction::AddProp);
    for p in &view.props {
        let (_, name, says) = PROPS.iter().find(|(k, ..)| *k == p.spec.kind).copied().unwrap_or(("", "Prop", ""));
        let r = ui.boxed(c, d, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), h: Some(1.9), pad: Some([0.0, 0.6, 0.0, 0.8].into()), ..UiStyle::row() }.outline(Tone::Fg5, 1.0));
        portrait(c, ui, look, r, 1.5);
        let tx = ui.boxed(c, r, UiStyle { gap: Some(0.2), ..UiStyle::column() });
        ui.text(c, tx, Role::DisplayRegular, 0.64, Tone::Fg, name);
        ui.text(c, tx, Role::Mono, 0.4, Tone::Fg3, says);
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let state = match p.group {
            Some((n, spread, mean)) => format!("{n} hits \u{00b7} {:.0} cm spread \u{00b7} {:.0} cm mean", spread * 100.0, mean * 100.0),
            None if p.standing.1 > 1 => format!("{} of {} up", p.standing.0, p.standing.1),
            None => String::new(),
        };
        ui.text(c, r, Role::Mono, 0.42, Tone::Fg3, &state);
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg2, &format!("{} m", p.spec.distance_m));
        let reset = ui.button(c, r, Kind::Ghost, if p.group.is_some() { "Clear" } else { "Reset" }, None, 1.5, State::NONE);
        c.entity(reset).insert(BenchAction::ResetProp(p.id));
        trash(c, ui, look, r, BenchAction::RemoveProp(p.id));
    }
    // Options.
    section(c, ui, d, "Options", "");
    for o in [SandboxOption::EndlessAmmo, SandboxOption::NoDamage, SandboxOption::BlastRings, SandboxOption::HalfSpeed, SandboxOption::ThirdPerson] {
        let on = options.get(o);
        let r = ui.boxed(c, d, UiStyle { align: Some(AlignItems::Center), h: Some(1.6), pad: Some([0.0, 0.3, 0.0, 0.3].into()), ..UiStyle::row() });
        ui.hairline(c, r, Tone::Fg5, 1.0);
        let label = if o == SandboxOption::BlastRings { "Blast rings after an explosion" } else { o.label() };
        ui.text(c, r, Role::DisplayRegular, 0.58, Tone::Fg, label);
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        small_toggle(c, ui, r, on, BenchAction::Option(o, !on));
    }
    ui.boxed(c, d, UiStyle { grow: Some(1.0), ..UiStyle::column() });
    let foot = ui.boxed(c, d, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), h: Some(3.0), shrink: Some(0.0), ..UiStyle::row() });
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(look.hair()), ..default() }, BackgroundColor(look.tone(Tone::Fg5)), ChildOf(foot)));
    ui.sentence(c, foot, 0.6, Tone::Fg2, "Changes apply at once. A target comes back 3 s after it falls.");
    ui.boxed(c, foot, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    button(c, ui, foot, "Reset bench", None, BenchAction::Reset);
}

fn short_game(name: &str) -> String {
    match name {
        "Counter-Strike: Source" => "CS:S".into(),
        "Combat Arms" => "CA".into(),
        "Half-Life 2: Deathmatch" => "HL2:DM".into(),
        "Halo: Combat Evolved" => "Halo".into(),
        "Team Fortress 2" => "TF2".into(),
        o => o.into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn picker(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, every: &[CharacterEntry], target: Option<u64>, chosen: Option<&str>, view: &BenchView, bench: &BenchUi, lists: &mut chimera_ui::VirtualLists) {
    let typed = bench.picker_search.as_str();
    let found = bench.found(every);
    let chars: Vec<CharacterEntry> = found.into_iter().cloned().collect();
    let chars = chars.as_slice();
    let scrim = c
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(ui.color(Tone::Ink, 0.6)),
            FocusPolicy::Block,
            Interaction::default(),
            ZIndex(10),
            ChildOf(root),
        ))
        .id();
    let plate = c
        .spawn((
            UiStyle { gap: Some(0.8), pad: Some([1.6, 1.8, 1.4, 1.8].into()), ..UiStyle::column() }.abs([Some(9.0), Some(7.2), Some(9.0), Some(2.4)]).bg(Tone::Ink2).outline(Tone::Fg4, 1.0),
            Node::default(),
            UiParityId::new("bench.picker"),
            ChildOf(scrim),
        ))
        .id();
    c.spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: ui.units.val(0.22), ..default() }, BackgroundColor(ui.color(Tone::Fg, 1.0)), ChildOf(plate)));
    let head = ui.boxed(c, plate, UiStyle { gap: Some(1.2), align: Some(AlignItems::Baseline), ..UiStyle::row() });
    let n = target.and_then(|id| view.targets.iter().position(|t| t.id == id)).unwrap_or(view.targets.len()) + 1;
    ui.text(c, head, Role::DisplayBold, 1.6, Tone::Fg, &format!("Target {n:02}"));
    ui.text(c, head, Role::Mono, 0.52, Tone::Fg2, "Who stands there, and what it holds");
    let body = ui.boxed(c, plate, UiStyle { gap: Some(1.2), grow: Some(1.0), align: Some(AlignItems::Stretch), ..UiStyle::row() });
    c.entity(body).insert(Node { min_height: Val::Px(0.0), ..default() });
    // Games.
    let games = ui.boxed(c, body, UiStyle { w: Some(13.0), gap: Some(0.1), shrink: Some(0.0), ..UiStyle::column() });
    ui.text(c, games, Role::Mono, 0.46, Tone::Fg3, "Games");
    let mut order: Vec<&str> = Vec::new();
    for ch in chars {
        if !order.contains(&ch.game.as_str()) {
            order.push(&ch.game);
        }
    }
    let all = ui.boxed(c, games, UiStyle { pad: Some([0.45, 0.7, 0.45, 0.7].into()), h: Some(2.0), align: Some(AlignItems::Center), ..UiStyle::row() }.bg(Tone::Fg));
    ui.text(c, all, Role::DisplayRegular, 0.64, Tone::Ink, "All games");
    ui.boxed(c, all, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    ui.text(c, all, Role::Mono, 0.54, Tone::Ink, &chars.len().to_string());
    for g in &order {
        let r = ui.boxed(c, games, UiStyle { pad: Some([0.45, 0.7, 0.45, 0.7].into()), h: Some(2.0), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.text(c, r, Role::DisplayRegular, 0.64, Tone::Fg2, g);
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.54, Tone::Fg3, &chars.iter().filter(|x| x.game == *g).count().to_string());
    }
    // The characters by game.
    let mid = ui.boxed(c, body, UiStyle { grow: Some(1.0), gap: Some(0.4), align: Some(AlignItems::Stretch), ..UiStyle::column() });
    // The kit's text field (a caret, a selection, copy and paste), the
    // count beside it.
    let find_row = ui.boxed(c, mid, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() });
    let byte = |i: usize| typed.char_indices().nth(i).map_or(typed.len(), |(b, _)| b);
    let cur = chimera_ui::controls::TextCursor { at: byte(bench.picker_caret), anchor: byte(bench.picker_anchor) };
    let find = ui.text_field(c, find_row, typed, "Find a character or a game", Some(cur), false, 30.0, State::focused(true));
    c.entity(find).insert((PickerSearch, UiParityId::new("bench.picker.find")));
    if !typed.is_empty() {
        ui.text(c, find_row, Role::Mono, 0.42, Tone::Fg3, &format!("{} of {}", chars.len(), every.len()));
    }
    if chars.is_empty() {
        ui.sentence(c, mid, 0.7, Tone::Fg3, "No character or game holds that.");
    }
    // The characters by game: the kit's recycling list inside the fixed
    // dialog (it scrolls; the buttons stay put).
    let by_game: Vec<(String, Vec<CharacterEntry>)> = order.iter().map(|g| (g.to_string(), chars.iter().filter(|x| x.game == *g).cloned().collect())).collect();
    {
        let st = lists.state(PICKER_LIST);
        st.set_shape(chimera_ui::VShape::grid(PICK_COLS, 1.5, PICK_TILE_H, PICK_GAP));
        st.set_sections(by_game.iter().map(|(g, v)| chimera_ui::VSection::new(g.clone(), v.len())).collect());
        st.bump();
    }
    let list = ui.virtual_list(c, mid, PICKER_LIST, UiStyle { grow: Some(1.0), ..UiStyle::row() }, 40.0, chimera_ui::VShape::grid(PICK_COLS, 1.5, PICK_TILE_H, PICK_GAP));
    c.entity(list.scroll.frame).insert(Node { min_height: Val::Px(0.0), ..default() });
    c.entity(mid).insert(Node { min_height: Val::Px(0.0), min_width: Val::Px(0.0), ..default() });
    for &slot in &list.slots {
        c.entity(slot).insert(Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, display: Display::None, column_gap: ui.units.val(PICK_GAP), ..default() });
    }
    c.insert_resource(PickerGrid { games: by_game, chosen: chosen.map(str::to_string) });
    // The chosen one.
    let side = ui.boxed(c, body, UiStyle { w: Some(17.0), gap: Some(0.35), pad: Some([0.0, 0.0, 0.0, 1.2].into()), shrink: Some(0.0), ..UiStyle::column() });
    if let Some(ch) = chosen.and_then(|id| every.iter().find(|x| x.id == id)) {
        portrait(c, ui, look, side, 8.0);
        ui.text(c, side, Role::Mono, 0.44, Tone::Fg3, &ch.game);
        ui.text(c, side, Role::DisplayBold, 1.2, Tone::Fg, &ch.name);
        let kv = |c: &mut Commands, k: &str, v: String| {
            let r = ui.boxed(c, side, UiStyle { h: Some(1.6), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.hairline(c, r, Tone::Fg5, 1.0);
            ui.text(c, r, Role::Mono, 0.42, Tone::Fg3, k);
            ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
            ui.text(c, r, Role::Mono, 0.5, Tone::Fg, &v);
        };
        kv(c, "Protection", if ch.shield > 0.0 { format!("Shield {} \u{00b7} health {}", ch.shield, ch.health) } else { format!("Health {}", ch.health) });
        kv(c, "Against other games", format!("About {} hit points", (ch.shield + ch.health).round()));
        let s = section(c, ui, side, "Holds", "");
        let _ = s;
        let r = ui.boxed(c, side, UiStyle { h: Some(1.7), gap: Some(0.6), align: Some(AlignItems::Center), pad: Some([0.0, 0.5, 0.0, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.08));
        ui.focus_bar(c, r, 0.2);
        ui.text(c, r, Role::DisplayRegular, 0.62, Tone::Fg, &ch.default_gun);
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.4, Tone::Fg3, "Its default");
    }
    let foot = ui.boxed(c, plate, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
    ui.sentence(c, foot, 0.76, Tone::Fg2, "A target holds its own game's default gun. It fires only with Shoots back on.");
    ui.boxed(c, foot, UiStyle { grow: Some(1.0), ..UiStyle::row() });
    let cancel = ui.button(c, foot, Kind::Secondary, "Cancel", Some("Esc"), 2.4, State::NONE);
    c.entity(cancel).insert(BenchAction::PickerCancel);
    let name = chosen.and_then(|id| every.iter().find(|x| x.id == id)).map_or("Use".to_string(), |ch| format!("Use {}", ch.name));
    let st = if chosen.is_some() { State::NONE } else { State { disabled: true, ..State::NONE } };
    let use_ = ui.button(c, foot, Kind::Primary, &name, Some("Enter"), HERO_H, st);
    c.entity(use_).insert(BenchAction::PickerUse);
    let _ = tag;
}

/// Made-up characters for the stagings.
pub fn staged_characters() -> CharacterCatalogue {
    let e = |id: &str, name: &str, game: &str, shield: f32, health: f32, gun: &str, bytes: Option<u64>| CharacterEntry {
        id: id.into(),
        name: name.into(),
        game: game.into(),
        shield,
        health,
        default_gun: gun.into(),
        guns: Vec::new(),
        bytes,
    };
    CharacterCatalogue(vec![
        e("halo_ce:spartan", "Spartan", "Halo: Combat Evolved", 100.0, 100.0, "Assault Rifle", None),
        e("combat_arms:soldier", "Soldier", "Combat Arms", 0.0, 100.0, "M4A1", None),
        e("combat_arms:recon", "Recon", "Combat Arms", 0.0, 100.0, "MP7", None),
        e("cs_source:phoenix", "Phoenix Connexion", "Counter-Strike: Source", 0.0, 100.0, "AK-47", None),
        e("cs_source:leet", "Elite Crew", "Counter-Strike: Source", 0.0, 100.0, "AK-47", None),
        e("cs_source:arctic", "Arctic Avengers", "Counter-Strike: Source", 0.0, 100.0, "AK-47", None),
        e("cs_source:guerilla", "Guerilla Warfare", "Counter-Strike: Source", 0.0, 100.0, "AK-47", None),
        e("cs_source:urban", "Seal Team 6", "Counter-Strike: Source", 0.0, 100.0, "M4A1", None),
        e("cs_source:gsg9", "GSG-9", "Counter-Strike: Source", 0.0, 100.0, "M4A1", None),
        e("cs_source:sas", "SAS", "Counter-Strike: Source", 0.0, 100.0, "M4A1", None),
        e("cs_source:gign", "GIGN", "Counter-Strike: Source", 0.0, 100.0, "M4A1", None),
        e("tf2:heavy", "Heavy", "Team Fortress 2", 0.0, 300.0, "Minigun", Some(8_200_000)),
        e("tf2:soldier", "Soldier", "Team Fortress 2", 0.0, 200.0, "Rocket Launcher", Some(7_900_000)),
        e("tf2:pyro", "Pyro", "Team Fortress 2", 0.0, 175.0, "Flame Thrower", Some(7_600_000)),
        e("tf2:demoman", "Demoman", "Team Fortress 2", 0.0, 175.0, "Grenade Launcher", Some(7_700_000)),
    ])
}

/// The picker's search field.
#[derive(Component, Clone, Copy, Debug)]
pub struct PickerSearch;

/// The pointer in the picker's search field (`controls::TextPress`): a
/// press places the caret, a drag selects, a double press selects all.
fn search_pointer(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    fields: Query<(Entity, &Interaction), With<PickerSearch>>,
    hits: chimera_ui::controls::TextHits,
    clock: Option<Res<chimera_ui::UiClock>>,
    mut press: Local<chimera_ui::controls::TextPress>,
    mut bench: ResMut<BenchUi>,
) {
    let Some(mouse) = mouse else { return };
    if bench.picker.is_none() {
        press.release();
        return;
    }
    let now = clock.map_or(0.0, |c| c.now);
    let set = |bench: &mut BenchUi, c: usize, a: usize| {
        let n = bench.picker_search.chars().count();
        let (c, a) = (c.min(n), a.min(n));
        if (bench.picker_caret, bench.picker_anchor) != (c, a) {
            bench.picker_caret = c;
            bench.picker_anchor = a;
        }
    };
    if mouse.just_pressed(MouseButton::Left) {
        let Some(e) = fields.iter().find_map(|(e, i)| (*i == Interaction::Pressed).then_some(e)) else {
            press.release();
            return;
        };
        let (Some(at), Some(len)) = (hits.char_at(e), hits.len(e)) else { return };
        let (c, a) = press.press(now, at, len);
        set(&mut bench, c, a);
    } else if mouse.pressed(MouseButton::Left) {
        let Some((e, _)) = fields.iter().next() else { return };
        let Some(at) = hits.char_at(e) else { return };
        if let Some((c, a)) = press.drag(at) {
            set(&mut bench, c, a);
        }
    } else {
        press.release();
    }
}

/// The picker's characters as built, by game, for the list's slots.
#[derive(Resource, Clone, Debug, Default)]
pub struct PickerGrid {
    pub games: Vec<(String, Vec<CharacterEntry>)>,
    pub chosen: Option<String>,
}

/// The picker's list key (`VirtualLists`, `ScrollViews`).
pub const PICKER_LIST: &str = "bench_picker";
const PICK_COLS: usize = 8;
const PICK_TILE_H: f32 = 7.2;
const PICK_GAP: f32 = 0.35;

/// Fills the picker's list slots: a game's header, a row of tiles.
#[allow(clippy::too_many_arguments)]
fn bind_picker(
    look: Look,
    lists: Res<chimera_ui::VirtualLists>,
    grid: Res<PickerGrid>,
    slots: Query<(Entity, &chimera_ui::VSlot), Changed<chimera_ui::VSlot>>,
    owners: Query<&chimera_ui::vlist::VirtualList>,
    mut c: Commands,
) {
    use chimera_ui::{VCursor, vlist::VLine};
    let Some(st) = lists.get(PICKER_LIST) else { return };
    let ui = Ui::new(&look.theme, look.fonts.as_deref(), *look.units);
    for (slot, s) in &slots {
        // Only this list's slots (every list's slots change on one query).
        if !owners.get(s.list).is_ok_and(|l| l.key == PICKER_LIST) {
            continue;
        }
        c.entity(slot).despawn_children();
        let Some(line) = s.line.and_then(|l| st.layout.lines.get(l)).copied() else { continue };
        match line {
            VLine::Header(g) => {
                let Some((name, _)) = grid.games.get(g) else { continue };
                let h = ui.boxed(&mut c, slot, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), grow: Some(1.0), ..UiStyle::row() });
                kit_icon(&mut c, &look, h, "chevron", 0.5, Tone::Fg2, 1.0);
                ui.text(&mut c, h, Role::Mono, 0.5, Tone::Fg2, name);
            }
            VLine::Row { section, first, count } => {
                for col in 0..PICK_COLS {
                    let cell = c.spawn((Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, ChildOf(slot))).id();
                    if col >= count {
                        continue;
                    }
                    let Some(ch) = grid.games.get(section).and_then(|g| g.1.get(first + col)) else { continue };
                    let _ = VCursor::Item(section, first + col);
                    let on = grid.chosen.as_deref() == Some(ch.id.as_str());
                    let t = c
                        .spawn((
                            UiStyle { gap: Some(0.3), pad: Some([0.3, 0.3, 0.3, 0.3].into()), grow: Some(1.0), min_w: Some(0.0), ..UiStyle::column() }
                                .bg_a(Tone::Fg, if on { 0.1 } else { 0.03 })
                                .outline(if on { Tone::Fg2 } else { Tone::Fg5 }, 1.0),
                            Node { min_width: Val::Px(0.0), overflow: Overflow::clip_x(), ..default() },
                            BenchAction::PickerChoose(ch.id.clone()),
                            clickable(),
                            ChildOf(cell),
                        ))
                        .id();
                    if on {
                        ui.underline(&mut c, t, Tone::Acc, 0.22);
                    }
                    let p = ui.boxed(&mut c, t, UiStyle { h: Some(3.4), justify: Some(JustifyContent::Center), align: Some(AlignItems::FlexEnd), ..UiStyle::row() }.bg(Tone::Ink3));
                    kit_icon(&mut c, &look, p, "profile", 2.8, if ch.bytes.is_some() { Tone::Fg4 } else { Tone::Fg3 }, 1.0);
                    if let Some(b) = ch.bytes {
                        let dl = ui.boxed(&mut c, p, UiStyle { gap: Some(0.25), align: Some(AlignItems::Center), pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.abs([None, Some(0.3), Some(0.3), None]).bg_a(Tone::Ink, 0.8).outline(Tone::Fg5, 1.0));
                        kit_icon(&mut c, &look, dl, "download", 0.45, Tone::Fg2, 1.0);
                        ui.text(&mut c, dl, Role::Mono, 0.36, Tone::Fg2, &super::units::size_words(b));
                    }
                    let nm = ui.text(&mut c, t, Role::DisplayRegular, 0.5, Tone::Fg, &ch.name);
                    c.entity(nm).insert(chimera_ui::Ellipsize::in_parent(ch.name.to_uppercase()));
                }
            }
        }
    }
}

/// The bench's systems.
pub fn add(app: &mut App) {
    app.init_resource::<BenchUi>().init_resource::<CharacterCatalogue>().init_resource::<PickerGrid>().add_message::<BenchAction>().add_systems(
        PostUpdate,
        bind_picker
            .after(chimera_ui::vlist::place_slots)
            .before(bevy::ui::UiSystems::Prepare)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    ).add_systems(
        Update,
        (keys, search_pointer, clicks, act, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    );
}
