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
//!   `protected`), Distance (5 to 200 m),
//!   Moves (Still, Strafes, Walks), Shoots back (its own toggle, off
//!   unless you want it). Every change is a `link::BenchRequest` at once.
//!   A refused spawn's reason shows under the list.
//! - **Props** (for accuracy; ADD PROP cycles the kinds): bottles, tin
//!   cans, a target board, each at its distance, with a trash can.
//! - **Options**: the sandbox's toggles (`link::SandboxOptions`); Take no
//!   damage and Third person send their console commands (`god`,
//!   `thirdperson`), the others are the mechanics' to read.
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
//! Keys: ↑↓ the target, N adds one, Delete removes the selected one,
//! Esc (or T) closes.
//!
//! Known shortcuts: portraits are the kit's profile mark (no generated
//! per-asset portraits yet, design rules section 21); the picker's search
//! field and the Holds list don't type or open yet; the drawer is rebuilt
//! whole on a change.

use bevy::{prelude::*, ui::{DeprecatedInteraction as Interaction, FocusPolicy}};
use chimera_ui::{
    Role, Tone, UiParityId, UiStyle,
    controls::{HERO_H, Kind, State, Ui, clickable},
};

use super::{
    SandboxScreen, SandboxUi,
    hud::{M, kit_icon, tag},
    link::{BenchRequest, BenchView, DISTANCES, MAX_TARGETS, PROPS, PropSpec, SandboxOption, SandboxOptions, TargetMotion, TargetSpec},
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
}

/// A click on the bench.
#[derive(Component, Message, Clone, Debug, PartialEq)]
pub enum BenchAction {
    Select(usize),
    Add,
    Remove(u64),
    Distance(u64, f32),
    Motion(u64, TargetMotion),
    Protected(u64, bool),
    Armored(u64, bool),
    ShootsBack(u64, bool),
    Character(Option<u64>),
    PickerChoose(String),
    PickerUse,
    PickerCancel,
    AddProp,
    RemoveProp(u64),
    Option(SandboxOption, bool),
    Reset,
    Close,
}

#[derive(Component)]
struct BenchRoot;

/// The drawer's width, cq.
pub const W: f32 = 31.0;

fn keys(keys: Res<ButtonInput<KeyCode>>, ui: Res<SandboxUi>, bench: Res<BenchUi>, view: Res<BenchView>, mut out: MessageWriter<BenchAction>) {
    if ui.screen != SandboxScreen::Bench {
        return;
    }
    let k = |c: KeyCode| keys.just_pressed(c);
    if bench.picker.is_some() {
        if k(KeyCode::Enter) {
            out.write(BenchAction::PickerUse);
        }
        return;
    }
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
            BenchAction::Select(i) => world.resource_mut::<BenchUi>().selected = i,
            BenchAction::Add => {
                if view.targets.len() < MAX_TARGETS {
                    world.resource_mut::<BenchUi>().picker = Some((None, None));
                }
            }
            BenchAction::Remove(id) => req(world, BenchRequest::Remove(id)),
            BenchAction::Distance(id, d) => {
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { distance_m: d, ..s }));
                }
            }
            BenchAction::Motion(id, m) => {
                if let Some(s) = spec_of(id) {
                    req(world, BenchRequest::Set(id, TargetSpec { motion: m, ..s }));
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
                match target.and_then(|id| spec_of(id).map(|s| (id, s))) {
                    Some((id, s)) => req(world, BenchRequest::Set(id, TargetSpec { character: ch, gun: None, ..s })),
                    None => req(world, BenchRequest::Spawn(TargetSpec { character: ch, ..default() })),
                }
            }
            BenchAction::PickerCancel => world.resource_mut::<BenchUi>().picker = None,
            BenchAction::AddProp => {
                let mut b = world.resource_mut::<BenchUi>();
                let (kind, ..) = PROPS[b.next_prop % PROPS.len()];
                b.next_prop += 1;
                let d = [20.0, 10.0, 25.0][(b.next_prop - 1) % 3];
                req(world, BenchRequest::AddProp(PropSpec { kind: kind.into(), distance_m: d }));
            }
            BenchAction::RemoveProp(id) => req(world, BenchRequest::RemoveProp(id)),
            BenchAction::Option(o, on) => {
                let staged = world.resource::<SandboxUi>().staged;
                let was = world.resource::<SandboxOptions>().get(o);
                if was != on {
                    world.resource_mut::<SandboxOptions>().set(o, on);
                    if !staged
                        && let Some(cmd) = SandboxOptions::command(o, on)
                        && let Some(mut c) = world.get_resource_mut::<chimera_core::console::Console>()
                    {
                        c.submit(cmd);
                    }
                }
            }
            BenchAction::Reset => req(world, BenchRequest::Clear),
            BenchAction::Close => {
                world.resource_mut::<SandboxUi>().screen = SandboxScreen::None;
            }
        }
    }
    // Esc: the picker first.
    let picker_open = world.resource::<BenchUi>().picker.is_some();
    let esc = world.get_resource::<ButtonInput<KeyCode>>().is_some_and(|k| k.just_pressed(KeyCode::Escape));
    if picker_open && esc && world.resource::<SandboxUi>().screen == SandboxScreen::Bench {
        world.resource_mut::<BenchUi>().picker = None;
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
    root: Query<Entity, With<BenchRoot>>,
    mut last: Local<Shown>,
    mut c: Commands,
) {
    let next = Shown {
        open: look.on() && ui_state.on && ui_state.screen == SandboxScreen::Bench,
        view: view.clone(),
        bench: bench.clone(),
        options: *options,
        chars: chars.0.len(),
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
    drawer(&mut c, &ui, &look, full, &view, &bench, &options);
    if let Some((target, chosen)) = &bench.picker {
        picker(&mut c, &ui, &look, full, &chars.0, *target, chosen.as_deref(), &view);
    }
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

fn drawer(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, view: &BenchView, bench: &BenchUi, options: &SandboxOptions) {
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
        let tx = ui.boxed(c, head, UiStyle { gap: Some(0.25), ..UiStyle::column() });
        ui.text(c, tx, Role::DisplayRegular, 0.74, Tone::Fg, &t.name);
        ui.text(c, tx, Role::Mono, 0.42, Tone::Fg3, &t.game);
        ui.boxed(c, head, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let sum = match t.respawn {
            Some(_) => "Down \u{00b7} back soon".to_string(),
            None => format!("{} m \u{00b7} {}", t.spec.distance_m, t.spec.motion.label()),
        };
        ui.text(c, head, Role::Mono, 0.46, Tone::Fg2, &sum);
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
        let f = ui.boxed(c, r, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), grow: Some(1.0), h: Some(1.45), pad: Some([0.0, 0.5, 0.0, 0.5].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.04));
        ui.underline(c, f, Tone::Fg3, 0.05);
        ui.text(c, f, Role::DisplayRegular, 0.64, Tone::Fg, t.spec.gun.as_deref().unwrap_or("Its default"));
        ui.boxed(c, f, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        kit_icon(c, look, f, "listchev", 0.6, Tone::Fg2, 1.0);
        let r = row(c, "Protection");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg2, if t.spec.armored { "Shield or gear \u{00b7} health" } else { "Health only" });
        small_toggle(c, ui, r, t.spec.armored, BenchAction::Armored(t.id, !t.spec.armored));
        let r = row(c, "Never falls");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg3, if t.spec.protected { "Hits measured, it stays up" } else { "Falls, back in 3 s" });
        small_toggle(c, ui, r, t.spec.protected, BenchAction::Protected(t.id, !t.spec.protected));
        let r = row(c, "Distance");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let words: Vec<String> = DISTANCES.iter().map(|d| format!("{d} m")).collect();
        let refs: Vec<&str> = words.iter().map(String::as_str).collect();
        let sel = DISTANCES.iter().position(|d| (*d - t.spec.distance_m).abs() < 0.01);
        for (k, e) in ui.segmented(c, r, &refs, sel, State::NONE).into_iter().enumerate() {
            c.entity(e).insert((BenchAction::Distance(t.id, DISTANCES[k]), clickable()));
        }
        let r = row(c, "Moves");
        ui.boxed(c, r, UiStyle { grow: Some(1.0), ..UiStyle::row() });
        let names: Vec<&str> = TargetMotion::ALL.iter().map(|m| m.label()).collect();
        let sel = TargetMotion::ALL.iter().position(|m| *m == t.spec.motion);
        for (k, e) in ui.segmented(c, r, &names, sel, State::NONE).into_iter().enumerate() {
            c.entity(e).insert((BenchAction::Motion(t.id, TargetMotion::ALL[k]), clickable()));
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
        ui.text(c, r, Role::Mono, 0.44, Tone::Fg2, &format!("{} m", p.spec.distance_m));
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
fn picker(c: &mut Commands, ui: &Ui, look: &Look, root: Entity, chars: &[CharacterEntry], target: Option<u64>, chosen: Option<&str>, view: &BenchView) {
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
    let body = ui.boxed(c, plate, UiStyle { gap: Some(1.2), grow: Some(1.0), ..UiStyle::row() });
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
    let mid = ui.boxed(c, body, UiStyle { grow: Some(1.0), gap: Some(0.4), ..UiStyle::column() });
    let find = ui.boxed(c, mid, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), h: Some(2.1), pad: Some([0.0, 0.8, 0.0, 0.8].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.05));
    kit_icon(c, look, find, "search", 0.8, Tone::Fg3, 1.0);
    ui.sentence(c, find, 0.78, Tone::Fg3, "Find a character or a game");
    for g in &order {
        let h = ui.boxed(c, mid, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), h: Some(1.5), ..UiStyle::row() });
        kit_icon(c, look, h, "chevron", 0.5, Tone::Fg2, 1.0);
        ui.text(c, h, Role::Mono, 0.5, Tone::Fg2, g);
        let row = c.spawn((UiStyle { gap: Some(0.35), ..UiStyle::row() }, Node { flex_wrap: FlexWrap::Wrap, row_gap: ui.units.val(0.35), ..default() }, ChildOf(mid))).id();
        for ch in chars.iter().filter(|x| x.game == *g) {
            let on = chosen == Some(ch.id.as_str());
            let t = c
                .spawn((
                    UiStyle { w: Some(5.2), gap: Some(0.3), pad: Some([0.3, 0.3, 0.3, 0.3].into()), ..UiStyle::column() }.bg_a(Tone::Fg, if on { 0.1 } else { 0.03 }).outline(if on { Tone::Fg2 } else { Tone::Fg5 }, 1.0),
                    Node::default(),
                    BenchAction::PickerChoose(ch.id.clone()),
                    clickable(),
                    ChildOf(row),
                ))
                .id();
            if on {
                ui.underline(c, t, Tone::Acc, 0.22);
            }
            let p = ui.boxed(c, t, UiStyle { h: Some(3.4), justify: Some(JustifyContent::Center), align: Some(AlignItems::FlexEnd), ..UiStyle::row() }.bg(Tone::Ink3));
            kit_icon(c, look, p, "profile", 2.8, if ch.bytes.is_some() { Tone::Fg4 } else { Tone::Fg3 }, 1.0);
            if let Some(b) = ch.bytes {
                let dl = ui.boxed(c, p, UiStyle { gap: Some(0.25), align: Some(AlignItems::Center), pad: Some([0.12, 0.25, 0.12, 0.25].into()), ..UiStyle::row() }.abs([None, Some(0.3), Some(0.3), None]).bg_a(Tone::Ink, 0.8).outline(Tone::Fg5, 1.0));
                kit_icon(c, look, dl, "download", 0.45, Tone::Fg2, 1.0);
                ui.text(c, dl, Role::Mono, 0.36, Tone::Fg2, &super::units::size_words(b));
            }
            ui.text(c, t, Role::DisplayRegular, 0.5, Tone::Fg, &ch.name);
        }
    }
    // The chosen one.
    let side = ui.boxed(c, body, UiStyle { w: Some(17.0), gap: Some(0.35), pad: Some([0.0, 0.0, 0.0, 1.2].into()), shrink: Some(0.0), ..UiStyle::column() });
    if let Some(ch) = chosen.and_then(|id| chars.iter().find(|x| x.id == id)) {
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
    let name = chosen.and_then(|id| chars.iter().find(|x| x.id == id)).map_or("Use".to_string(), |ch| format!("Use {}", ch.name));
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

/// The bench's systems.
pub fn add(app: &mut App) {
    app.init_resource::<BenchUi>().init_resource::<CharacterCatalogue>().add_message::<BenchAction>().add_systems(
        Update,
        (keys, clicks, act, draw)
            .chain()
            .after(super::SandboxSystems)
            .run_if(resource_exists::<chimera_ui::Theme>)
            .run_if(resource_exists::<chimera_ui::UiUnits>),
    );
}
