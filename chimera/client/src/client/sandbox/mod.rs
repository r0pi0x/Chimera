//! The sandbox's screens (Ultimate Gun Game's `sandbox`: a match alone on
//! every game's guns, `chimera_sim::minigame::rung_command::HoldRungs`),
//! in the Signal kit (the design rules, sections 18 to 21, revision 2 as
//! decided): its HUD, Change gun, the bench and what the range shows.
//! The mechanics (the rung switch, targets, measurement, blasts) are the
//! mechanics side's; these screens read and ask through `link` and
//! `units`.
//!
//! - `model`: the gun catalogue with its tags, the Change gun view's
//!   filters, the pins (plain data).
//! - `units`: downloads on demand (`UnitFetcher`, `UnitProgress`).
//! - `link`: the mechanics' side as the screens see it (targets, kills,
//!   the looked-at target, blasts, options), with a fake.
//! - `hud`: the HUD at rest (the mark and B and T hints, the gun chip with
//!   its condition tags or Next strip, the pinned bar) and with Tab held
//!   (one column at the left: the last kill and the four before it, the
//!   options, the keys).
//! - `change_gun`: the full-screen tile view (B).
//! - `bench`: the bench drawer (T): targets, props, options, and the
//!   character picker.
//! - `range`: over the world: the look panel on the target under the
//!   crosshair, respawn rings, blast rings.
//!
//! **State** (`SandboxUi`): whether a sandbox plays (`on`; or a staging),
//! the screen open (`SandboxScreen`), Tab held, the pins and the bar's
//! waking, the Change gun view, the gun in hand and the one picked that
//! waits for its download (`pending`), a drag of a tile. `SandboxCatalogue`
//! is the guns, made when a sandbox's ladder comes up (`catalogue_of`).
//!
//! **Keys** (`keys`, only while a sandbox plays and neither the console
//! nor chat has the keys): B Change gun, T the bench (each toggles; Esc
//! closes), 1 to 6 switch to a pinned gun (and show the bar), K repeats
//! the last set-up, Tab held
//! (`+showscores`) the Tab column. The same as console commands:
//! `sandbox_gun [gun]` (with a gun: opened on it), `sandbox_bench`,
//! `sandbox_pin <1-6> [gun]`, `sandbox_key <1-6|k>` (scenes).
//!
//! **Switching** (`switch_to`): a gun whose files are here is asked of
//! the mechanics (`rung_command::PutOnGun`, as the class the detail pane
//! chose for a TF2 gun several classes carry; a refusal, `RungRefused`,
//! shows on the chip; a network client sends `rung <gun> [as <body>]`);
//! K asks for the last set-up again (`layout::RepeatLayout`); the
//! mechanics' conditions (`conditions::SandboxConditions`) are mirrored
//! into the options shown. One not here is asked for
//! (`UnitRequest`) and waits as `pending`, shown on the chip's Next
//! strip, switched to when it lands.
//!
//! `ui_scene sandbox <rest|tab|pins|next|gun|gun-detail|gun-tags|gun-fold|drag|bench|bench-pick|range|off>`
//! stages the screens with made-up guns, kills and downloads (`stage`).
//!
//! Known shortcuts: B, T and 1 to 6 are read as keys, not binds, while a
//! sandbox plays (rebinding them is not offered yet); the games' full
//! names are a table here (`game_name`), the registry has short ones only.

pub mod bench;
pub mod change_gun;
pub mod hud;
pub mod link;
pub mod model;
pub mod range;
pub mod units;

use bevy::{ecs::system::SystemParam, prelude::*};
use chimera_core::{console::ConsoleAppExt, game_parts::GunClass};
use chimera_sim::{
    minigame::{
        ActiveMinigame,
        ladder::{Ladder, Rung},
        rung_command::{self, HoldRungs},
    },
    weapon::data::WeaponCatalog,
};

use self::{
    link::{BenchRequest, BenchView, BlastMeasured, KillLog, LookPanel, SandboxOptions},
    model::{Files, GunView, PinBar, Pins, SandboxGun},
    units::{UnitProgress, UnitRequest, UnitState},
};
use crate::core::LocalPlayer;

/// The screen open over the range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SandboxScreen {
    #[default]
    None,
    ChangeGun,
    Bench,
}

/// A tile dragged toward the pinned bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub gun: String,
    /// The pointer, logical px.
    pub at: Vec2,
    /// The slot under it (0-based).
    pub over: Option<usize>,
}

/// The sandbox's client state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SandboxUi {
    pub on: bool,
    /// Staged by `ui_scene` (nothing switches for real).
    pub staged: bool,
    pub screen: SandboxScreen,
    pub tab: bool,
    pub pins: Pins,
    pub pin_bar: PinBar,
    pub view: GunView,
    /// The gun in hand.
    pub current: Option<String>,
    /// Picked, downloading before it's in hand.
    pub pending: Option<String>,
    pub drag: Option<Drag>,
    /// A word for the chip's foot (a switch refused).
    pub said: Option<String>,
    /// A sandbox screen took Esc this frame (the Escape menu leaves it).
    pub took_esc: bool,
}

impl SandboxUi {
    /// A screen owns the mouse and keys.
    pub fn screen_open(&self) -> bool {
        self.on && self.screen != SandboxScreen::None
    }
}

/// The guns (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SandboxCatalogue {
    pub guns: Vec<SandboxGun>,
    /// The ladder it was made from (its rungs), to make it again on change.
    made_from: Vec<&'static str>,
}

impl SandboxCatalogue {
    pub fn new(guns: Vec<SandboxGun>) -> Self {
        SandboxCatalogue { guns, made_from: Vec::new() }
    }

    pub fn get(&self, id: &str) -> Option<&SandboxGun> {
        self.guns.iter().find(|g| g.id == id)
    }

    /// A gun's files: here, not here, downloading.
    pub fn files(g: &SandboxGun, progress: &UnitProgress) -> Files {
        match g.unit.as_deref().map(|u| progress.state(&g.game, u)) {
            None | Some(UnitState::Here) => Files::Here,
            Some(UnitState::Queued { .. } | UnitState::Downloading { .. }) => Files::Downloading,
            Some(_) => Files::NotHere,
        }
    }

    /// A gun's download state (here without a unit).
    pub fn unit_state(g: &SandboxGun, progress: &UnitProgress) -> UnitState {
        g.unit.as_deref().map_or(UnitState::Here, |u| progress.state(&g.game, u))
    }
}

/// A game's full name by its id.
pub fn game_name(id: &str) -> String {
    match id {
        "cs_source" => "Counter-Strike: Source".into(),
        "combat_arms" => "Combat Arms".into(),
        "hl2dm" => "Half-Life 2: Deathmatch".into(),
        "halo_ce" => "Halo: Combat Evolved".into(),
        "tf2" => "Team Fortress 2".into(),
        "" => "Other".into(),
        other => other.replace('_', " "),
    }
}

/// A game's short name by its id (the registry's, else a table).
pub fn game_short(id: &str, games: Option<&chimera_core::game_registry::Games>) -> String {
    if let Some(i) = games.and_then(|r| r.get(id)) {
        return i.short_name.to_string();
    }
    match id {
        "cs_source" => "CS:S",
        "combat_arms" => "CA",
        "hl2dm" => "HL2:DM",
        "halo_ce" => "Halo",
        "tf2" => "TF2",
        other => other,
    }
    .to_string()
}

/// The catalogue of `rungs` (ids, in the ladder's order) with what
/// `world` knows of each gun (module doc of `model`).
pub fn catalogue_of(world: &World, rungs: &[&str]) -> Vec<SandboxGun> {
    let mut classes: std::collections::BTreeMap<&str, GunClass> = Default::default();
    for c in chimera_sim::minigame::auto::report(world) {
        for g in &c.guns {
            classes.insert(g.id, g.class);
        }
        for g in &c.grenades {
            classes.insert(g, GunClass::Grenade);
        }
    }
    let catalog = world.get_resource::<WeaponCatalog>();
    let names = world.get_resource::<crate::client::minigame_scoreboard::GunNames>();
    let games = world.get_resource::<chimera_core::game_registry::Games>();
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (i, id) in rungs.iter().enumerate() {
        if !seen.insert(*id) {
            continue;
        }
        let data = catalog.and_then(|c| c.get(id));
        let class = classes.get(id).copied().or_else(|| data.and_then(|d| d.class));
        let (tags, game_tags, stats) = model::derive_tags(id, data, class);
        let game = id.split_once(':').map_or("", |(g, _)| g).to_string();
        out.push(SandboxGun {
            id: id.to_string(),
            rung: i,
            game_name: game_name(&game),
            game_short: game_short(&game, games),
            name: crate::client::minigame_scoreboard::gun_name(id, names, catalog),
            class,
            icon_class: data.and_then(crate::client::signal::icon_class),
            tags,
            game_tags,
            stats,
            unit: chimera_net::net::units::gun_unit(id).map(|(_, set)| set),
            game,
        });
    }
    out
}

/// Whether a sandbox plays.
#[derive(SystemParam)]
pub struct SandboxPlays<'w> {
    hold: Option<Res<'w, HoldRungs>>,
    active: Option<Res<'w, ActiveMinigame>>,
}

impl SandboxPlays<'_> {
    pub fn on(&self) -> bool {
        matches!((self.hold.as_deref(), self.active.as_deref()), (Some(h), Some(a)) if a.def.id == h.0)
    }
}

/// Keeps `on`, the gun in hand and the catalogue (module doc).
fn track(world: &mut World) {
    let staged = world.get_resource::<SandboxUi>().is_some_and(|s| s.staged);
    if staged {
        return;
    }
    let on = matches!(
        (world.get_resource::<HoldRungs>(), world.get_resource::<ActiveMinigame>()),
        (Some(h), Some(a)) if a.def.id == h.0
    );
    // Nothing to do outside a sandbox (the ladder isn't even looked at).
    if !on && !world.get_resource::<SandboxUi>().is_some_and(|s| s.on || s.current.is_some()) {
        return;
    }
    let ladder = if on { world.get_resource::<Ladder>().cloned() } else { None };
    let current = if on {
        let rung = world.query_filtered::<&Rung, With<LocalPlayer>>().iter(world).next().map(|r| r.0);
        rung.and_then(|r| ladder.as_ref()?.shown_weapon(r, world.get_resource::<WeaponCatalog>())).map(str::to_string)
    } else {
        None
    };
    if on && let Some(ladder) = ladder.as_ref() {
        // The sentinel (the last rung) is never listed.
        let usable = &ladder.rungs[..ladder.rungs.len().saturating_sub(1)];
        if world.get_resource::<SandboxCatalogue>().is_none_or(|c| c.made_from != usable) {
            let guns = catalogue_of(world, usable);
            // Each gun's unit: here, or what it would download.
            let units: Vec<(String, String)> =
                guns.iter().filter_map(|g| Some((g.game.clone(), g.unit.clone()?))).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            chimera_net::net::units::describe_units(world, &units);
            world.insert_resource(SandboxCatalogue { guns, made_from: usable.to_vec() });
        }
    }
    let mut ui = world.resource_mut::<SandboxUi>();
    if ui.on != on {
        ui.on = on;
        if !on {
            ui.screen = SandboxScreen::None;
            ui.drag = None;
        }
    }
    if ui.current != current {
        ui.current = current;
    }
}

/// Puts the local player on `gun` (module doc, "Switching").
pub fn switch_to(world: &mut World, gun: &str) -> Result<(), String> {
    let cat = world.get_resource::<SandboxCatalogue>().ok_or("no sandbox plays")?;
    let g = cat.get(gun).ok_or_else(|| format!("no gun {gun} in the sandbox"))?.clone();
    let progress = world.get_resource::<UnitProgress>().cloned().unwrap_or_default();
    match SandboxCatalogue::unit_state(&g, &progress) {
        UnitState::Here => {}
        UnitState::NotHere { .. } | UnitState::Failed { .. } => {
            if let Some(u) = &g.unit {
                let failed = matches!(SandboxCatalogue::unit_state(&g, &progress), UnitState::Failed { .. });
                world.write_message(if failed {
                    UnitRequest::Retry { game: g.game.clone(), unit: u.clone() }
                } else {
                    UnitRequest::Fetch { game: g.game.clone(), unit: u.clone() }
                });
            }
            world.resource_mut::<SandboxUi>().pending = Some(g.id.clone());
            return Ok(());
        }
        _ => {
            world.resource_mut::<SandboxUi>().pending = Some(g.id.clone());
            return Ok(());
        }
    }
    let mut ui = world.resource_mut::<SandboxUi>();
    ui.pending = None;
    if ui.staged {
        ui.current = Some(g.id.clone());
        return Ok(());
    }
    // As a body of its game (a TF2 gun several classes carry: the class
    // the detail pane chose).
    let body = ui.view.body.clone().filter(|b| b.split_once(':').is_some_and(|(game, _)| game == g.game));
    if world.get_resource::<chimera_core::core::NetRole>() == Some(&chimera_core::core::NetRole::Client) {
        let line = match &body {
            Some(b) => format!("rung {} as {b}", g.id),
            None => format!("rung {}", g.id),
        };
        world.resource_mut::<chimera_core::console::Console>().submit(line);
        return Ok(());
    }
    // The mechanics' request (`rung_command::PutOnGun`; a refusal comes
    // back as `RungRefused`, shown on the chip).
    world.write_message(rung_command::PutOnGun { gun: g.id.clone(), character: body, who: None });
    Ok(())
}

/// The mechanics' refusals (a switch, a condition) on the chip's foot.
fn refusals(
    mut rung: Option<MessageReader<rung_command::RungRefused>>,
    mut cond: Option<MessageReader<chimera_sim::minigame::conditions::ConditionRefused>>,
    mut ui: ResMut<SandboxUi>,
) {
    let a = rung.as_mut().and_then(|r| r.read().last().map(|r| r.reason.clone()));
    let b = cond.as_mut().and_then(|r| r.read().last().map(|r| r.reason.clone()));
    if let Some(why) = a.or(b) {
        ui.said = Some(why);
    }
}

/// The mechanics' conditions into the options the screens show; God's
/// brief flip at a spawn is ignored (it must hold `GOD_SETTLE` s).
pub const GOD_SETTLE: f64 = 0.5;

fn mirror_conditions(
    ui: Res<SandboxUi>,
    clock: Option<Res<chimera_ui::UiClock>>,
    conditions: Option<Res<chimera_sim::minigame::conditions::SandboxConditions>>,
    mut options: ResMut<SandboxOptions>,
    mut god_since: Local<Option<(bool, f64)>>,
) {
    let Some(c) = conditions else { return };
    if !ui.on || ui.staged {
        return;
    }
    let now = clock.map_or(0.0, |c| c.now);
    let mut next = *options;
    next.endless_ammo = c.endless_ammo;
    next.half_speed = c.time_scale < 0.99;
    next.third_person = c.third_person;
    match *god_since {
        Some((g, _)) if g == c.god => {}
        _ => *god_since = Some((c.god, now)),
    }
    if let Some((g, t)) = *god_since
        && now - t >= GOD_SETTLE
    {
        next.no_damage = g;
    }
    if *options != next {
        *options = next;
    }
}

/// A gun picked while its files downloaded: switched to when they land.
fn land_pending(world: &mut World) {
    let Some(gun) = world.get_resource::<SandboxUi>().and_then(|s| s.pending.clone()) else { return };
    let here = world.get_resource::<SandboxCatalogue>().and_then(|c| c.get(&gun)).is_some_and(|g| {
        SandboxCatalogue::unit_state(g, world.get_resource::<UnitProgress>().unwrap_or(&UnitProgress::default())).is_here()
    });
    if here {
        let _ = switch_to(world, &gun);
    }
}

/// What the keys did this frame (for the world-side actions).
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SandboxAction {
    /// Switch to a gun (a pinned slot, a double click, Enter).
    Switch(String),
    Toggle(SandboxScreen),
    Close,
    /// K: the last set-up again (`layout::RepeatLayout`).
    Repeat,
    /// Put Change gun's cursor on a gun (`sandbox_gun <id>`).
    Select(String),
}

/// The keys (module doc).
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    held: Option<Res<crate::client::console::HeldActions>>,
    console: Option<Res<crate::client::console::ConsoleUi>>,
    chat: Option<Res<crate::client::chat::ChatInput>>,
    clock: Option<Res<chimera_ui::UiClock>>,
    bench: Option<Res<bench::BenchUi>>,
    mut ui: ResMut<SandboxUi>,
    mut out: MessageWriter<SandboxAction>,
) {
    if !ui.on {
        return;
    }
    // The bench's character picker takes letters for its search.
    let picking = ui.screen == SandboxScreen::Bench && bench.is_some_and(|b| b.picker.is_some());
    if ui.took_esc {
        ui.took_esc = false;
    }
    let tab = held.is_some_and(|h| h.scores());
    if ui.tab != tab && !ui.staged {
        ui.tab = tab;
    }
    if console.is_some_and(|c| c.open) || chat.is_some_and(|c| c.open.is_some()) {
        return;
    }
    let now = clock.map_or(0.0, |c| c.now);
    let k = |c: KeyCode| keys.just_pressed(c);
    // Change gun's own keys are its own (typing, arrows): only Esc and B
    // reach here while it's open.
    let typing = (ui.screen == SandboxScreen::ChangeGun && !ui.view.search.is_empty()) || picking;
    if k(KeyCode::Escape) && ui.screen == SandboxScreen::Bench {
        ui.took_esc = true;
        out.write(SandboxAction::Close);
        return;
    }
    if !typing && ui.screen != SandboxScreen::ChangeGun && k(KeyCode::KeyB) {
        out.write(SandboxAction::Toggle(SandboxScreen::ChangeGun));
    }
    if !picking && ui.screen != SandboxScreen::ChangeGun && k(KeyCode::KeyT) {
        out.write(SandboxAction::Toggle(SandboxScreen::Bench));
    }
    if ui.screen == SandboxScreen::None && k(KeyCode::KeyK) {
        out.write(SandboxAction::Repeat);
    }
    if ui.screen == SandboxScreen::None {
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        for (i, d) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6].into_iter().enumerate() {
            if k(d) && !ctrl {
                ui.pin_bar.wake(now);
                if let Some(g) = ui.pins.0[i].clone() {
                    out.write(SandboxAction::Switch(g));
                }
            }
        }
    }
    if tab {
        ui.pin_bar.wake(now);
    }
}

/// The actions on the world.
fn act(world: &mut World) {
    let actions: Vec<SandboxAction> = world.resource_mut::<Messages<SandboxAction>>().drain().collect();
    for a in actions {
        match a {
            SandboxAction::Switch(g) => {
                if let Err(e) = switch_to(world, &g) {
                    world.resource_mut::<SandboxUi>().said = Some(e);
                }
            }
            SandboxAction::Toggle(s) => {
                let mut ui = world.resource_mut::<SandboxUi>();
                ui.screen = if ui.screen == s { SandboxScreen::None } else { s };
                ui.drag = None;
                if ui.screen == SandboxScreen::ChangeGun {
                    ui.view.selected = ui.current.clone();
                    ui.view.tags_open = false;
                }
            }
            SandboxAction::Select(id) => {
                let mut ui = world.resource_mut::<SandboxUi>();
                ui.view.selected = Some(id);
            }
            SandboxAction::Repeat => {
                if !world.resource::<SandboxUi>().staged && world.contains_resource::<Messages<chimera_sim::minigame::layout::RepeatLayout>>() {
                    world.write_message(chimera_sim::minigame::layout::RepeatLayout { by: None });
                }
            }
            SandboxAction::Close => {
                let mut ui = world.resource_mut::<SandboxUi>();
                if ui.view.tags_open {
                    ui.view.tags_open = false;
                } else {
                    ui.screen = SandboxScreen::None;
                    ui.drag = None;
                }
            }
        }
    }
}

/// The sandbox's systems (in `Update`; the screens draw after them).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SandboxSystems;

/// The screens (module doc).
pub struct SandboxUiPlugin;

impl Plugin for SandboxUiPlugin {
    fn build(&self, app: &mut App) {
        crate::client::signal::setup(app);
        app.init_resource::<SandboxUi>()
            .init_resource::<SandboxCatalogue>()
            .init_resource::<UnitProgress>()
            .init_resource::<units::RefusedUnits>()
            .init_resource::<BenchView>()
            .init_resource::<KillLog>()
            .init_resource::<LookPanel>()
            .init_resource::<SandboxOptions>()
            .init_resource::<link::HitsSoFar>()
            .init_resource::<link::LastHits>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<SandboxAction>()
            .add_message::<UnitRequest>()
            .add_message::<BenchRequest>()
            .add_message::<BlastMeasured>()
            .add_systems(
                Update,
                (
                    track,
                    keys,
                    act,
                    units::poll_units,
                    units::bridge_units,
                    link::fake_bench,
                    link::bridge_requests,
                    link::bridge_view,
                    link::bridge_characters,
                    link::bridge_facts,
                    link::bridge_look,
                    link::bridge_blasts,
                    refusals,
                    mirror_conditions,
                    land_pending,
                )
                    .chain()
                    .in_set(SandboxSystems),
            );
        crate::client::ui_scene::UiSceneHooks::add(app.world_mut(), "sandbox", stage);
        hud::add(app);
        change_gun::add(app);
        bench::add(app);
        range::add(app);
        app.console_command(
            "chimera:sandbox_gun",
            "sandbox_gun [gun id]: open or close the sandbox's Change gun (B); with a gun, open it on that gun (its detail).",
            |w, a| {
                match a.first() {
                    Some(id) => {
                        let open = w.resource::<SandboxUi>().screen == SandboxScreen::ChangeGun;
                        if !open {
                            w.write_message(SandboxAction::Toggle(SandboxScreen::ChangeGun));
                        }
                        w.write_message(SandboxAction::Select(id.clone()));
                    }
                    None => {
                        w.write_message(SandboxAction::Toggle(SandboxScreen::ChangeGun));
                    }
                }
                Ok(None)
            },
        )
        .console_command("chimera:sandbox_key", "sandbox_key <1-6|k>: what that key does while a sandbox plays (a pinned gun, K's set-up again; scenes).", |w, a| {
            let k = a.first().map(|s| s.to_ascii_lowercase()).ok_or("sandbox_key <1-6|k>")?;
            let now = w.get_resource::<chimera_ui::UiClock>().map_or(0.0, |c| c.now);
            if k == "k" {
                w.write_message(SandboxAction::Repeat);
                return Ok(None);
            }
            let slot = k.parse::<usize>().ok().filter(|s| (1..=model::PIN_SLOTS).contains(s)).ok_or("sandbox_key <1-6|k>")?;
            let gun = {
                let mut ui = w.resource_mut::<SandboxUi>();
                ui.pin_bar.wake(now);
                ui.pins.0[slot - 1].clone()
            };
            if let Some(g) = gun {
                w.write_message(SandboxAction::Switch(g));
            }
            Ok(None)
        })
        .console_command("chimera:sandbox_bench", "sandbox_bench: open or close the sandbox's bench (T).", |w, _| {
            w.write_message(SandboxAction::Toggle(SandboxScreen::Bench));
            Ok(None)
        })
        .console_command(
            "chimera:sandbox_pin",
            "sandbox_pin <1-6> [gun id]: pin a gun to a slot of the sandbox's pinned bar (the gun in hand without one); \
             `sandbox_pin <1-6> -` empties it.",
            |w, a| {
                let slot = a.first().and_then(|s| s.parse::<usize>().ok()).filter(|s| (1..=model::PIN_SLOTS).contains(s)).ok_or("sandbox_pin <1-6> [gun]")?;
                let mut ui = w.resource_mut::<SandboxUi>();
                match a.get(1).map(String::as_str) {
                    Some("-") => ui.pins.unpin(slot - 1),
                    Some(g) => ui.pins.pin(slot - 1, g),
                    None => {
                        let g = ui.current.clone().ok_or("no gun in hand")?;
                        ui.pins.pin(slot - 1, &g);
                    }
                }
                Ok(None)
            },
        );
    }
}

// ---------------------------------------------------------------------
// Stagings.

/// Made-up guns for the stagings (id, name, game, class, kit icon, unit
/// size in bytes: 0 here).
const STAGED: &[(&str, &str, &str, Option<GunClass>, &str, u64)] = &[
    ("cs_source:weapon_glock", "Glock-18", "cs_source", Some(GunClass::Pistol), "pistol", 0),
    ("cs_source:weapon_deagle", "Desert Eagle", "cs_source", Some(GunClass::HeavyPistol), "pistol", 0),
    ("combat_arms:m92fs", "M92FS", "combat_arms", Some(GunClass::Pistol), "pistol", 0),
    ("hl2dm:weapon_357", ".357 Magnum", "hl2dm", Some(GunClass::HeavyPistol), "revolver", 0),
    ("halo_ce:pistol", "Pistol", "halo_ce", Some(GunClass::Pistol), "pistol", 22_500_000),
    ("halo_ce:plasma_pistol", "Plasma Pistol", "halo_ce", Some(GunClass::Pistol), "energy", 22_500_000),
    ("cs_source:weapon_mp5navy", "MP5", "cs_source", Some(GunClass::Smg), "smg", 0),
    ("combat_arms:mp7", "MP7", "combat_arms", Some(GunClass::Smg), "smg", 0),
    ("hl2dm:weapon_smg1", "SMG1", "hl2dm", Some(GunClass::Smg), "smg", 0),
    ("cs_source:weapon_ak47", "AK-47", "cs_source", Some(GunClass::Rifle), "rifle", 0),
    ("cs_source:weapon_m4a1", "M4A1", "cs_source", Some(GunClass::Rifle), "rifle", 0),
    ("combat_arms:m4a1", "M4A1", "combat_arms", Some(GunClass::Rifle), "rifle", 0),
    ("combat_arms:m60", "M60", "combat_arms", Some(GunClass::Heavy), "lmg", 0),
    ("hl2dm:weapon_ar2", "Pulse Rifle", "hl2dm", Some(GunClass::Rifle), "energy", 0),
    ("halo_ce:assault_rifle", "Assault Rifle", "halo_ce", Some(GunClass::Rifle), "rifle", 22_500_000),
    ("cs_source:weapon_m3", "M3 Super 90", "cs_source", Some(GunClass::Shotgun), "shotgun", 0),
    ("hl2dm:weapon_shotgun", "Shotgun", "hl2dm", Some(GunClass::Shotgun), "shotgun", 0),
    ("tf2:tf_weapon_scattergun", "Scattergun", "tf2", Some(GunClass::Shotgun), "shotgun", 0),
    ("tf2:tf_weapon_shotgun_primary", "Shotgun", "tf2", Some(GunClass::Shotgun), "shotgun", 3_900_000),
    ("cs_source:weapon_awp", "AWP", "cs_source", Some(GunClass::Sniper), "sniper", 0),
    ("combat_arms:dragunov", "Dragunov SVD", "combat_arms", Some(GunClass::Sniper), "sniper", 0),
    ("combat_arms:l96a1", "L96A1", "combat_arms", Some(GunClass::Sniper), "sniper", 0),
    ("hl2dm:weapon_crossbow", "Crossbow", "hl2dm", Some(GunClass::Sniper), "crossbow", 0),
    ("halo_ce:sniper_rifle", "Sniper Rifle", "halo_ce", Some(GunClass::Sniper), "sniper", 22_500_000),
    ("tf2:tf_weapon_sniperrifle", "Sniper Rifle", "tf2", Some(GunClass::Sniper), "sniper", 6_400_000),
    ("combat_arms:m32", "M32", "combat_arms", Some(GunClass::Launcher), "launcher", 0),
    ("halo_ce:rocket_launcher", "Rocket Launcher", "halo_ce", Some(GunClass::Launcher), "launcher", 22_500_000),
    ("tf2:tf_weapon_minigun", "Minigun", "tf2", Some(GunClass::Heavy), "lmg", 5_100_000),
    ("tf2:tf_weapon_rocketlauncher", "Rocket Launcher", "tf2", Some(GunClass::Launcher), "launcher", 4_800_000),
    ("tf2:tf_weapon_grenadelauncher", "Grenade Launcher", "tf2", Some(GunClass::Launcher), "launcher", 2_500_000),
    ("tf2:tf_weapon_pipebomblauncher", "Stickybomb Launcher", "tf2", Some(GunClass::Launcher), "launcher", 6_200_000),
    ("tf2:tf_weapon_flamethrower", "Flame Thrower", "tf2", Some(GunClass::Flamethrower), "launcher", 5_600_000),
    ("cs_source:weapon_hegrenade", "HE Grenade", "cs_source", Some(GunClass::Grenade), "grenade", 0),
    ("halo_ce:frag_grenade", "Frag Grenade", "halo_ce", Some(GunClass::Grenade), "grenade", 22_500_000),
    ("combat_arms:knife", "Knife", "combat_arms", None, "knife", 0),
    ("tf2:tf_weapon_bat", "Bat", "tf2", None, "knife", 0),
];

/// The TF2 classes that carry each staged TF2 gun.
fn staged_facets(id: &str) -> Vec<String> {
    let c: &[&str] = match id {
        "tf2:tf_weapon_scattergun" => &["Scout"],
        "tf2:tf_weapon_shotgun_primary" => &["Soldier", "Pyro", "Heavy", "Engineer"],
        "tf2:tf_weapon_sniperrifle" => &["Sniper"],
        "tf2:tf_weapon_minigun" => &["Heavy"],
        "tf2:tf_weapon_rocketlauncher" => &["Soldier"],
        "tf2:tf_weapon_grenadelauncher" | "tf2:tf_weapon_pipebomblauncher" => &["Demoman"],
        "tf2:tf_weapon_flamethrower" => &["Pyro"],
        "tf2:tf_weapon_bat" => &["Scout"],
        _ => &[],
    };
    c.iter().map(|s| s.to_string()).collect()
}

/// The staged catalogue (with the world's data where it has the gun).
pub fn staged_catalogue(world: &World) -> Vec<SandboxGun> {
    let catalog = world.get_resource::<WeaponCatalog>();
    STAGED
        .iter()
        .enumerate()
        .map(|(i, (id, name, game, class, icon, bytes))| {
            let data = catalog.and_then(|c| c.get(id));
            let (mut tags, mut facets, stats) = model::derive_tags(id, data, *class);
            if data.is_none() {
                // No data here: a plausible delivery for the look.
                let d = match class {
                    Some(GunClass::Launcher) => "projectile",
                    Some(GunClass::Grenade) => "thrown",
                    Some(GunClass::Flamethrower) => "stream",
                    None => "melee",
                    _ => "hitscan",
                };
                tags.insert((model::TagGroup::Delivery, d));
                if class.is_none() {
                    tags.insert((model::TagGroup::Type, "melee"));
                }
                if matches!(class, Some(GunClass::Smg | GunClass::Rifle | GunClass::Heavy)) {
                    tags.insert((model::TagGroup::FireMode, "auto"));
                }
            }
            // The classes that carry it (`arms` names one class only).
            let staged = staged_facets(id);
            if !staged.is_empty() {
                facets = staged;
            }
            SandboxGun {
                id: id.to_string(),
                rung: i,
                game: game.to_string(),
                game_name: game_name(game),
                game_short: game_short(game, None),
                name: name.to_string(),
                class: *class,
                icon_class: Some(icon),
                tags,
                game_tags: facets,
                stats,
                unit: (*bytes > 0).then(|| if *game == "halo_ce" { "halo_ce".to_string() } else { id.to_string() }),
            }
        })
        .collect()
}

/// Made-up kills for the Tab column.
fn staged_kills() -> KillLog {
    use link::{HitLine, KillCard, TargetMotion};
    let k = |gun: &str, name: &str, short: &str, victim: &str, prot: &str, hits: &[f32], t: f32| KillCard {
        gun: gun.into(),
        gun_name: name.into(),
        gun_game_short: short.into(),
        victim: victim.into(),
        protection: prot.into(),
        distance_m: 10.0,
        motion: TargetMotion::Still,
        shots: hits.len() as u32 + 1,
        hits: hits
            .iter()
            .enumerate()
            .map(|(i, d)| HitLine { damage: *d, group: if i == 2 { "Stomach" } else { "Chest" }.into(), t: t * i as f32 / (hits.len() - 1).max(1) as f32 })
            .collect(),
        first_hit_to_kill: t,
        at: 0.0,
    };
    let mut log = KillLog::default();
    log.push(k("halo_ce:pistol", "Desert Eagle", "CS:S", "Spartan", "Spartan", &[60.0, 60.0, 60.0], 0.45));
    log.push(k("hl2dm:weapon_ar2", "Pulse Rifle", "HL2:DM", "100-pt body", "100-pt body", &[15.0; 7], 0.6));
    log.push(k("hl2dm:weapon_smg1", "SMG1", "HL2:DM", "100-pt body", "100-pt body", &[8.0; 14], 0.9));
    log.push(k("cs_source:weapon_ak47", "AK-47", "CS:S", "Body", "CS:S body", &[36.0, 36.0, 36.0], 0.2));
    log.push(k("cs_source:weapon_ak47", "AK-47", "CS:S", "Spartan", "Shield and health", &[53.0, 53.0, 53.0, 53.0], 0.3));
    log
}

/// `ui_scene sandbox <what>` (module doc).
pub fn stage(w: &mut World, args: &[String]) -> Result<(), String> {
    let what = args.first().map(String::as_str).unwrap_or("rest");
    if what == "off" {
        if w.get_resource::<SandboxUi>().is_some_and(|s| s.staged) {
            w.insert_resource(SandboxUi::default());
            w.insert_resource(SandboxCatalogue::default());
            w.remove_resource::<units::UnitFetch>();
            w.insert_resource(UnitProgress::default());
            w.insert_resource(KillLog::default());
        }
        return Ok(());
    }
    let guns = staged_catalogue(w);
    let units: Vec<(String, String, u64)> = STAGED
        .iter()
        .filter(|s| s.5 > 0)
        .map(|s| (s.2.to_string(), if s.2 == "halo_ce" { "halo_ce".to_string() } else { s.0.to_string() }, s.5))
        .collect();
    let refs: Vec<(&str, &str, u64)> = units.iter().map(|(g, u, b)| (g.as_str(), u.as_str(), *b)).collect();
    let mut fake = units::FakeFetcher::new(&refs, 1_500_000.0);
    // Two of TF2's here, one downloading, one queued, one failed (the
    // Change gun mock's states).
    fake.set("tf2", "tf2:tf_weapon_minigun", UnitState::Here);
    fake.set("tf2", "tf2:tf_weapon_pipebomblauncher", UnitState::Downloading { done: 3_968_000, total: 6_200_000 });
    fake.set("tf2", "tf2:tf_weapon_flamethrower", UnitState::Queued { bytes: 5_600_000 });
    fake.set("tf2", "tf2:tf_weapon_shotgun_primary", UnitState::Failed { reason: "The server stopped answering.".into() });
    fake.rate = 0.0;
    let mut ui = SandboxUi { on: true, staged: true, current: Some("cs_source:weapon_ak47".into()), ..default() };
    for (i, g) in ["cs_source:weapon_ak47", "combat_arms:m4a1", "halo_ce:assault_rifle", "hl2dm:weapon_ar2", "tf2:tf_weapon_minigun"].iter().enumerate() {
        ui.pins.pin(i, g);
    }
    let now = w.get_resource::<chimera_ui::UiClock>().map_or(0.0, |c| c.now);
    ui.view.selected = ui.current.clone();
    match what {
        "rest" => {}
        "tab" => {
            ui.tab = true;
            ui.pin_bar.wake(now);
        }
        "pins" => ui.pin_bar.wake(now),
        "next" => {
            ui.pending = Some("tf2:tf_weapon_rocketlauncher".into());
            fake.set("tf2", "tf2:tf_weapon_rocketlauncher", UnitState::Downloading { done: 3_100_000, total: 4_800_000 });
        }
        "gun" | "gun-detail" | "gun-tags" | "gun-fold" | "drag" => {
            ui.screen = SandboxScreen::ChangeGun;
            ui.view.detail = what == "gun-detail";
            if what == "gun-tags" {
                ui.view.tags_open = true;
                ui.view.tags.insert((model::TagGroup::Delivery, "projectile"));
                ui.view.tags.insert((model::TagGroup::Behaviour, "explosive"));
            }
            if what == "gun-detail" {
                ui.view.games.insert("tf2".into());
                ui.view.selected = Some("tf2:tf_weapon_rocketlauncher".into());
            }
            if what == "drag" {
                ui.view.selected = Some("tf2:tf_weapon_rocketlauncher".into());
                ui.drag = Some(Drag { gun: "tf2:tf_weapon_rocketlauncher".into(), at: Vec2::new(1240.0, 975.0), over: Some(1) });
                ui.pin_bar.wake(now);
            }
        }
        "bench" | "bench-pick" => {
            ui.screen = SandboxScreen::Bench;
            ui.tab = false;
        }
        "range" => {}
        other => {
            return Err(format!(
                "ui_scene sandbox: no staging `{other}` (rest, tab, pins, next, gun, gun-detail, gun-tags, gun-fold, drag, bench, bench-pick, range)"
            ));
        }
    }
    let fold = what == "gun-fold";
    stage_bench(w, what);
    w.insert_resource(SandboxCatalogue { guns, made_from: Vec::new() });
    w.insert_resource(units::UnitFetch(Box::new(fake)));
    w.insert_resource(staged_kills());
    w.insert_resource(ui);
    {
        let mut lists = w.get_resource_or_init::<chimera_ui::VirtualLists>();
        let st = lists.state(change_gun::LIST);
        st.folded.clear();
        if fold {
            st.folded.insert("Pistols".into());
            st.folded.insert("SMGs".into());
            st.folded.insert("Shotguns".into());
        }
    }
    Ok(())
}

/// The bench's and the range's made-up targets, props and readouts.
fn stage_bench(w: &mut World, what: &str) {
    use link::{BenchView, LookedAt, PropRow, PropSpec, TargetMotion, TargetRow, TargetSpec};
    let t = |id: u64, ch: &str, d: f32, m: TargetMotion, respawn: Option<(f64, f32)>| {
        let (name, game) = link::character_words(ch);
        TargetRow { id, spec: TargetSpec { character: ch.into(), distance_m: d, motion: m, ..default() }, name, game, respawn, home: None }
    };
    let mut view = BenchView {
        targets: vec![
            t(1, "halo_ce:spartan", 10.0, TargetMotion::Still, None),
            t(2, "combat_arms:soldier", 20.0, TargetMotion::Strafes, None),
            t(3, "cs_source:phoenix_connexion", 100.0, TargetMotion::Walks, None),
        ],
        props: vec![
            PropRow { id: 11, spec: PropSpec { kind: "bottles".into(), distance_m: 20.0 }, standing: (4, 6), group: None },
            PropRow { id: 12, spec: PropSpec { kind: "tin_cans".into(), distance_m: 10.0 }, standing: (5, 5), group: None },
            PropRow { id: 13, spec: PropSpec { kind: "target_board".into(), distance_m: 25.0 }, standing: (1, 1), group: Some((7, 0.21, 0.08)) },
        ],
        refused: None,
    };
    let mut bench = bench::BenchUi::default();
    if what == "bench-pick" {
        bench.picker = Some((Some(1), Some("halo_ce:spartan".into())));
    }
    let mut panel = LookPanel::default();
    let mut blasts = range::Blasts::default();
    if what == "range" {
        view.targets[1].respawn = Some((0.0, 3.0));
        panel.0 = Some(LookedAt {
            name: "Spartan".into(),
            game_short: "Halo".into(),
            last: Some(53.0),
            total: 159.0,
            shield: Some((0.0, 100.0)),
            health: (41.0, 100.0),
            top: Vec3::ZERO,
            feet: Vec3::ZERO,
            recharging: true,
        });
        let now = w.get_resource::<chimera_ui::UiClock>().map_or(0.0, |c| c.now);
        blasts.0.push((now, BlastMeasured { at: Vec3::ZERO, inner: 0.0, outer: 3.7, max_damage: 90.0, edge_damage: 45.0, hits: vec![(Vec3::ZERO, 72.0)] }));
    }
    w.insert_resource(view);
    w.insert_resource(bench);
    w.insert_resource(panel);
    w.insert_resource(blasts);
    // Each body may hold its game's staged guns (the Holds list).
    let mut chars = bench::staged_characters();
    for ch in &mut chars.0 {
        ch.guns = STAGED.iter().filter(|g| game_name(g.2) == ch.game).map(|g| g.0.to_string()).collect();
    }
    w.insert_resource(chars);
    w.insert_resource(link::FakeBench::default());
}
