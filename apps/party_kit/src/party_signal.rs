//! The party panel, PROVISIONAL (docs/plans/active/parties.md, "The UI
//! (provisional)"): built from the kit's existing parts only (the modal,
//! buttons, rows, the text field), no design of its own; the user signs
//! off the real one. It draws `chimera_net::net::party::PartyClient` and
//! acts through the party's console commands.
//!
//! - **Open**: the shell rack's "+" (`ShellTile::Invite`, live with a hub
//!   account) or `party_panel [0|1]`; Esc or Close shuts it.
//! - **Not in a party**: *Create a party*, and a code field (typing goes
//!   to it while the panel is open: letters, digits and dashes,
//!   Backspace, Enter joins) with *Join*.
//! - **In one**: the invite code large, the members in join order (YOU,
//!   LEADER, offline, on a server), the leader's *Promote* and *Kick* per
//!   member, where the party is with *Join* when you're elsewhere, and
//!   *Leave party*.
//! - **The follow prompt** (`PartyClient::prompt`, panel shut): a card at
//!   the top, "Your party is on <server>", *Join* and *Stay*.
//! - `ui_scene party [in|lead|prompt|off]` stages it with a made-up
//!   party (screenshots).
//!
//! The menus behind it take no keys while it is open (`main_menu`'s
//! typing, the Signal menu's selection and pages' keys).
//!
//! Known shortcuts: no Copy (the code is read out); no keyboard focus
//! between the buttons; the panel rebuilds whole on any change.

use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};
use chimera_net::net::party::{MAX_MEMBERS, MemberView, PartyClient, PartyView, Target};
use chimera_ui::{
    Role, Tone, UiStyle,
    controls::{Kind, NOTE, SENTENCE, State, Ui},
};

/// Over the menus, under the report dialog and the console.
pub const PANEL_Z: i32 = 94;
const WIDTH: f32 = 44.0;

/// The panel's state.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartyPanel {
    pub open: bool,
    /// The code being typed.
    pub code: String,
}

/// A click on the panel or the prompt.
#[derive(Component, Message, Clone, Debug, PartialEq, Eq)]
pub enum PartyAction {
    Create,
    Join,
    Leave,
    Kick(String),
    Promote(String),
    /// Join the party's server.
    Follow,
    Stay,
    Close,
}

#[derive(Component)]
pub struct PanelRoot;

#[derive(Component)]
pub struct PromptRoot;

pub struct PartySignalPlugin;

impl Plugin for PartySignalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyPanel>().add_message::<PartyAction>();
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "party", scene_party);
        let _ = chimera_core::console::ConsoleAppExt::console_command(app, "party_panel", "party_panel [0|1]: the party panel (provisional).", |w, a| {
            let open = a.first().map_or(!w.resource::<PartyPanel>().open, |v| v != "0");
            w.resource_mut::<PartyPanel>().open = open;
            Ok(None)
        });
        app.add_systems(Update, (open_from_tile, clicks, typing, act, show, show_prompt).chain());
    }
}

#[allow(clippy::type_complexity)]
fn open_from_tile(
    tiles: Query<(&Interaction, &chimera_ui::shell::ShellTile), (Changed<Interaction>, Without<chimera_ui::shell::Disabled>)>,
    mut panel: ResMut<PartyPanel>,
) {
    if tiles.iter().any(|(i, t)| *i == Interaction::Pressed && *t == chimera_ui::shell::ShellTile::Invite) {
        panel.open = !panel.open;
    }
}

fn clicks(pressed: Query<(&Interaction, &PartyAction), Changed<Interaction>>, mut out: MessageWriter<PartyAction>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(a.clone());
        }
    }
}

/// The code field takes the keys while the panel is open and no party.
fn typing(
    mut keys: MessageReader<KeyboardInput>,
    mut panel: ResMut<PartyPanel>,
    party: Option<Res<PartyClient>>,
    guard: Option<ResMut<chimera_ui::shell::BackGuard>>,
    mut out: MessageWriter<PartyAction>,
) {
    if !panel.open {
        keys.clear();
        return;
    }
    if let Some(mut g) = guard {
        g.hold();
    }
    let in_party = party.is_some_and(|p| p.in_party());
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Escape => out.write(PartyAction::Close),
            Key::Enter if !in_party && !panel.code.is_empty() => out.write(PartyAction::Join),
            Key::Backspace if !in_party => {
                panel.code.pop();
                continue;
            }
            Key::Character(t) if !in_party => {
                for ch in t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-') {
                    if panel.code.len() < 9 {
                        panel.code.push(ch.to_ascii_uppercase());
                    }
                }
                continue;
            }
            _ => continue,
        };
    }
}

fn act(mut actions: MessageReader<PartyAction>, mut panel: ResMut<PartyPanel>, console: Option<ResMut<chimera_core::console::Console>>) {
    let Some(mut console) = console else {
        actions.clear();
        return;
    };
    for a in actions.read() {
        let line = match a {
            PartyAction::Create => "party_create".to_string(),
            PartyAction::Join => format!("party_join {}", std::mem::take(&mut panel.code)),
            PartyAction::Leave => "party_leave".to_string(),
            PartyAction::Kick(m) => format!("party_kick {m}"),
            PartyAction::Promote(m) => format!("party_promote {m}"),
            PartyAction::Follow => {
                panel.open = false;
                "party_follow".to_string()
            }
            PartyAction::Stay => "party_stay".to_string(),
            PartyAction::Close => {
                panel.open = false;
                continue;
            }
        };
        console.submit(line);
    }
}

/// What the panel last showed.
#[derive(PartialEq)]
struct Shown {
    panel: PartyPanel,
    view: Option<PartyView>,
    available: bool,
    busy: bool,
    error: Option<String>,
    note: Option<String>,
    at: Option<String>,
    /// The main menu's UI camera (drawn there when it is up).
    camera: Option<Entity>,
    units: (u32, u32),
}

#[allow(clippy::type_complexity)]
fn show(
    panel: Res<PartyPanel>,
    party: Option<Res<PartyClient>>,
    address: Option<Res<chimera_net::net::client::ServerAddress>>,
    camera: Query<Entity, With<crate::main_menu::MenuUiCamera>>,
    root: Query<Entity, With<PanelRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<Shown>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    if !panel.open {
        if shown.take().is_some() {
            for e in &root {
                commands.entity(e).despawn();
            }
        }
        return;
    }
    let p = party.as_deref();
    let now = Shown {
        panel: panel.clone(),
        view: p.and_then(|p| p.view.clone()),
        available: p.is_some_and(PartyClient::available),
        busy: p.is_some_and(|p| p.busy > 0 && p.view.is_none()),
        error: p.and_then(|p| p.error.clone()),
        note: p.and_then(|p| p.note.clone()),
        at: address.map(|a| a.0.to_string()),
        camera: camera.iter().next(),
        units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    let ui = Ui::new(theme, kit.as_deref(), *units);
    spawn(&mut commands, &ui, &now);
    *shown = Some(now);
}

fn button(c: &mut Commands, ui: &Ui, parent: Entity, kind: Kind, label: &str, key: Option<&str>, a: PartyAction) -> Entity {
    let h = if kind == Kind::Primary { chimera_ui::controls::HERO_H } else { 2.4 };
    let b = ui.button(c, parent, kind, label, key, h, State::NONE);
    c.entity(b).insert(a);
    b
}

fn spawn(c: &mut Commands, ui: &Ui, s: &Shown) {
    let root = c
        .spawn((
            PanelRoot,
            GlobalZIndex(PANEL_Z),
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
    if let Some(cam) = s.camera {
        c.entity(root).insert(UiTargetCamera(cam));
    }
    let m = ui.modal(c, root, WIDTH, false);
    let top = ui.boxed(c, m, UiStyle { gap: Some(0.8), margin: Some([0.5, 0.0, 1.0, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, top, Tone::Acc, 0.6, 0.9);
    ui.text(c, top, Role::DisplayBold, 1.35, Tone::Fg, "Party");
    ui.boxed(c, top, UiStyle::row().grow(1.0));
    ui.text(c, top, Role::Mono, 0.5, Tone::Fg3, "Provisional");
    match &s.view {
        None => not_in_party(c, ui, m, s),
        Some(v) => in_party(c, ui, m, v, s.at.as_deref()),
    }
    for (line, tone) in [(s.error.as_deref(), Tone::Threat), (s.note.as_deref(), Tone::Fg2)] {
        if let Some(l) = line {
            let n = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.8, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
            ui.mark(c, n, tone, 0.42, 0.6);
            ui.sentence(c, n, NOTE, tone, l);
        }
    }
    let foot = ui.boxed(c, m, UiStyle { gap: Some(0.5), margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    if s.view.is_some() {
        button(c, ui, foot, Kind::Danger, "Leave party", None, PartyAction::Leave);
    }
    ui.boxed(c, foot, UiStyle::row().grow(1.0));
    button(c, ui, foot, Kind::Secondary, "Close", Some("Esc"), PartyAction::Close);
}

fn not_in_party(c: &mut Commands, ui: &Ui, m: Entity, s: &Shown) {
    if !s.available {
        ui.paragraph(c, m, Role::Text, SENTENCE, Tone::Fg2, "Parties need an account on a hub. Sign in to a hub first.");
        return;
    }
    ui.paragraph(c, m, Role::Text, SENTENCE, Tone::Fg2, "Play together: make a party and share its code, or join a friend's with theirs.");
    let make = ui.boxed(c, m, UiStyle { margin: Some([1.2, 0.0, 1.2, 0.0].into()), ..UiStyle::row() });
    button(c, ui, make, Kind::Primary, if s.busy { "Making..." } else { "Create a party" }, None, PartyAction::Create);
    ui.section(c, m, "Join with a code", Some("Type the code you were given"));
    let row = ui.boxed(c, m, UiStyle { gap: Some(0.6), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    let at = s.panel.code.chars().count();
    ui.text_field(c, row, &s.panel.code, "ABC-123", Some(at.into()), false, 14.0, State::focused(true));
    button(c, ui, row, Kind::Secondary, "Join", Some("Enter"), PartyAction::Join);
}

fn member_row(c: &mut Commands, ui: &Ui, list: Entity, v: &PartyView, mm: &MemberView) {
    let mut small = Vec::new();
    if mm.id == v.you {
        small.push("You".to_string());
    }
    if mm.leader {
        small.push("Leader".into());
    }
    if !mm.online {
        small.push("Offline".into());
    } else if mm.at.is_some() {
        small.push("In a match".into());
    }
    let small = small.join(" · ");
    let (row, right) = ui.row(c, list, &mm.name, (!small.is_empty()).then_some(small.as_str()), 2.4, State::NONE);
    let _ = row;
    if v.leading() && mm.id != v.you {
        button(c, ui, right, Kind::Secondary, "Promote", None, PartyAction::Promote(mm.id.clone()));
        button(c, ui, right, Kind::Danger, "Kick", None, PartyAction::Kick(mm.id.clone()));
    }
}

fn in_party(c: &mut Commands, ui: &Ui, m: Entity, v: &PartyView, at: Option<&str>) {
    let code = ui.boxed(c, m, UiStyle { gap: Some(1.0), align: Some(AlignItems::FlexEnd), margin: Some([0.2, 0.0, 1.0, 0.0].into()), ..UiStyle::row() });
    ui.text(c, code, Role::DisplayBold, 2.6, Tone::Fg, &v.code);
    ui.sentence(c, code, NOTE, Tone::Fg3, "Invite code: give it to your friends");
    ui.section(c, m, &format!("Members {} / {MAX_MEMBERS}", v.members.len()), None);
    let list = ui.boxed(c, m, UiStyle { gap: Some(0.2), ..UiStyle::column() });
    for mm in &v.members {
        member_row(c, ui, list, v, mm);
    }
    if let Some(t) = &v.target {
        let w = ui.boxed(c, m, UiStyle { gap: Some(0.8), margin: Some([1.0, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.sentence(c, w, SENTENCE, Tone::Fg2, &format!("Your party is on {}", shown_name(t)));
        if at != Some(t.addr.as_str()) && !v.leading() {
            ui.boxed(c, w, UiStyle::row().grow(1.0));
            button(c, ui, w, Kind::Primary, "Join", None, PartyAction::Follow);
        }
    }
}

/// The server's name, else its address hidden (`chimera_ui::secret`).
fn shown_name(t: &Target) -> String {
    if t.name.trim().is_empty() { chimera_ui::secret::mask(&t.addr) } else { t.name.clone() }
}

/// The follow prompt at the top, while the panel is shut.
#[allow(clippy::type_complexity)]
fn show_prompt(
    panel: Res<PartyPanel>,
    party: Option<Res<PartyClient>>,
    camera: Query<Entity, With<crate::main_menu::MenuUiCamera>>,
    root: Query<Entity, With<PromptRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<(Target, (u32, u32), Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let live = party
        .and_then(|p| p.prompt.clone())
        .filter(|_| !panel.open)
        .map(|t| (t, ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), camera.iter().next()));
    if *shown == live {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((t, _, cam)) = &live {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let c = &mut commands;
        let r = c
            .spawn((
                PromptRoot,
                GlobalZIndex(PANEL_Z),
                Node { position_type: PositionType::Absolute, width: percent(100.0), top: ui.units.val(11.0), justify_content: JustifyContent::Center, ..default() },
                Pickable::IGNORE,
            ))
            .id();
        if let Some(cam) = cam {
            c.entity(r).insert(UiTargetCamera(*cam));
        }
        let b = ui.boxed(c, r, UiStyle { gap: Some(0.9), pad: Some([0.8, 1.4].into()), ..UiStyle::row() }.bg(Tone::Ink2).outline(Tone::Fg4, 1.0));
        ui.focus_bar(c, b, 0.22);
        ui.mark(c, b, Tone::Acc, 0.5, 0.7);
        ui.text(c, b, Role::Mono, 0.62, Tone::Fg, &format!("Your party is on {}", shown_name(t)));
        button(c, &ui, b, Kind::Primary, "Join", None, PartyAction::Follow);
        button(c, &ui, b, Kind::Secondary, "Stay", None, PartyAction::Stay);
    }
    *shown = live;
}

// --- Screenshots.

/// A hub that answers a staged party (`ui_scene party`).
struct StagedHub(PartyView);

impl chimera_net::net::party::PartyHub for StagedHub {
    fn origin(&self) -> String {
        "staged".into()
    }
    fn create(&self, _: &chimera_net::net::party::doc::Create) -> Result<chimera_net::net::party::doc::Joined, chimera_net::net::party::doc::Failure> {
        Ok(chimera_net::net::party::doc::Joined { token: "staged".into(), party: self.0.clone() })
    }
    fn join(&self, _: &chimera_net::net::party::doc::Join) -> Result<chimera_net::net::party::doc::Joined, chimera_net::net::party::doc::Failure> {
        Ok(chimera_net::net::party::doc::Joined { token: "staged".into(), party: self.0.clone() })
    }
    fn state(&self, _: &chimera_net::net::party::doc::State) -> Result<PartyView, chimera_net::net::party::doc::Failure> {
        Ok(self.0.clone())
    }
    fn leave(&self, _: &str) -> Result<(), chimera_net::net::party::doc::Failure> {
        Ok(())
    }
    fn kick(&self, _: &chimera_net::net::party::doc::OnMember) -> Result<PartyView, chimera_net::net::party::doc::Failure> {
        Ok(self.0.clone())
    }
    fn promote(&self, _: &chimera_net::net::party::doc::OnMember) -> Result<PartyView, chimera_net::net::party::doc::Failure> {
        Ok(self.0.clone())
    }
    fn lead(&self, _: &chimera_net::net::party::doc::Lead) -> Result<PartyView, chimera_net::net::party::doc::Failure> {
        Ok(self.0.clone())
    }
}

/// A made-up party of four, `you` the member asking.
pub fn staged_view(you: &str) -> PartyView {
    let m = |id: &str, name: &str, avatar: u8, online: bool, leader: bool, at: bool| MemberView {
        id: id.into(),
        name: name.into(),
        avatar,
        online,
        leader,
        at: at.then(|| "192.0.2.10:27031".into()),
    };
    PartyView {
        id: "a1b2c3d4e5f6".into(),
        code: "K7Q-4MX".into(),
        members: vec![m("00000001", "Player", 8, true, true, true), m("00000002", "Wraith", 19, true, false, true), m("00000003", "Nova", 27, true, false, false), m("00000004", "Basalt", 3, false, false, false)],
        you: you.into(),
        target: Some(Target { addr: "192.0.2.10:27031".into(), name: "Saturday playtest".into() }),
        moves: 1,
        kind: chimera_net::net::party::MoveKind::Join,
        left: None,
    }
}

/// `ui_scene party`: the panel with no party; `in` in a made-up party as
/// a member, `lead` as its leader; `prompt` the follow prompt; `off`.
fn scene_party(w: &mut World, args: &[String]) -> Result<(), String> {
    let arg = args.first().map(String::as_str);
    if arg == Some("off") {
        w.resource_mut::<PartyPanel>().open = false;
        if let Some(mut p) = w.get_resource_mut::<PartyClient>() {
            p.prompt = None;
        }
        return Ok(());
    }
    let you = if arg == Some("lead") { "00000001" } else { "00000003" };
    let view = staged_view(you);
    let mut pc = PartyClient::new(std::sync::Arc::new(StagedHub(view.clone())), None);
    if matches!(arg, Some("in" | "lead" | "prompt")) {
        pc.token = Some("staged".into());
        pc.view = Some(view.clone());
    }
    if arg == Some("prompt") {
        pc.prompt = view.target.clone();
    }
    w.insert_resource(pc);
    let mut panel = w.resource_mut::<PartyPanel>();
    panel.open = arg != Some("prompt");
    if arg.is_none() {
        panel.code = "K7Q-4".into();
    }
    Ok(())
}
