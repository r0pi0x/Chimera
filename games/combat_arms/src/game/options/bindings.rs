//! Controls and key binding (specs/combat_arms/options.md K): the
//! bindable actions, a binding's three device slots, the loading rules
//! for missing or empty bindings (K4), rebinding by capturing input (K3)
//! and the engine's side (the console's binds).
//!
//! - `ACTIONS` is the table of K1's 38 rows (the layout's row control,
//!   the profile's action name, the console command it runs) and the four
//!   actions without a row, with each action's built-in fallback key
//!   (`Fallback`).
//! - `Binding` is `"kbd@mouse@joystick"`; `BindingTable` holds one per
//!   action in table order; the profile's `[Bindings]` section is its
//!   file form (`load`, `store`).
//! - `Capture` is a row waiting for input: the first frame with any event
//!   erases the action's bindings, the first key-down decides (the wheel,
//!   or a remappable key stolen from every other action in the same slot).
//! - `apply` writes the engine's binds from a table: each key runs its
//!   actions' commands (two actions on one key run both), and keys 1 to 6
//!   also run `choose_1`..`choose_6` (bound for the session: config.cfg,
//!   shared with the other apps, doesn't save them,
//!   `Console::bound_for_session`).
//!
//! Known shortcuts: the commands of actions whose features aren't built
//! (recording, co-op, quests, the super-soldier item, night vision, the help popup's draw) are
//! bound all the same; a key press skips the ones nothing registers
//! (`COMMANDS_UNBUILT`, `client::binds::known_here`); the key a bind is "already
//! taken" by (K3's remappable rule) is a console bind that isn't one of
//! the options' own actions, as Tab's scoreboard; the joystick slot is
//! kept and shown but nothing reads it.

use bevy::prelude::*;

use super::keys::{self, KeyNames, RESERVED, WHEEL_DOWN, WHEEL_UP};
use crate::{console::Console, profile::Profile};

/// What an action gets back when it loads with no binding (K4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// No fallback: stays unbound.
    None,
    /// Always put back, even when another action holds the key.
    Always(&'static str),
    /// Put back when no earlier action holds the key; else the section
    /// is replaced by the default profile's (K4) or the action stays
    /// unbound.
    IfFree(&'static str),
}

/// A bindable action.
#[derive(Clone, Copy, Debug)]
pub struct Action {
    /// The layout's row control (K1); None: no row.
    pub row: Option<&'static str>,
    /// The action's name in `[Bindings]`.
    pub name: &'static str,
    /// What the key runs: a console command (`+x` for a held action).
    pub command: &'static str,
    pub fallback: Fallback,
}

const fn a(row: &'static str, name: &'static str, command: &'static str, fallback: Fallback) -> Action {
    Action {
        row: Some(row),
        name,
        command,
        fallback,
    }
}

const fn hidden(name: &'static str, command: &'static str, fallback: Fallback) -> Action {
    Action {
        row: None,
        name,
        command,
        fallback,
    }
}

use Fallback::{Always, IfFree, None as NoFallback};

/// K1's rows in the list's order, then the four actions without a row.
pub const ACTIONS: &[Action] = &[
    a("IncreaseSens", "SensitivityInc", "sensitivityinc", NoFallback),
    a("DecreaseSens", "SensitivityDec", "sensitivitydec", NoFallback),
    a("Forward", "Forward", "+forward", NoFallback),
    a("Back", "Backward", "+back", NoFallback),
    a("Left", "Left", "+moveleft", NoFallback),
    a("Right", "Right", "+moveright", NoFallback),
    a("Action", "Activate", "+use", NoFallback),
    a("Jump", "Jump", "+jump", NoFallback),
    a("Walk", "Speed", "+walk", NoFallback),
    a("Crouch", "DuckLock", "+duck", NoFallback),
    a("Sprint", "Dash", "+sprint", NoFallback),
    a("Fire", "Fire", "+attack", NoFallback),
    a("Reload", "Reload", "+reload", NoFallback),
    a("FiringMode", "TripleShotMode", "+firemode", Always("##33")),
    a("Zoomin", "ZoomIn", "+attack2", NoFallback),
    a("Map", "FullTacticalMap", "+fulltacticalmap", NoFallback),
    a("PreviousWeapon", "Prev_Weapon", "lastinv", Always("##16")),
    a("PrimaryWeapon", "PrimaryWeapon", "slot1", NoFallback),
    a("SecondaryWeapon", "SecondaryWeapon", "slot2", NoFallback),
    a("MeleeWeapon", "MeleeWeapon", "slot3", NoFallback),
    a("SupportItem", "MiscWeapon", "slot4", NoFallback),
    a("SpecialItem", "SpecialWeapon", "slot5", Always("##6")),
    a("MissionItem", "ObjectiveWeapon", "slot6", NoFallback),
    a("Chatting", "SendMessage", "chat_open", NoFallback),
    a("SocialMsg", "RadioRequest", "radio_request", NoFallback),
    a("TalkInGamePlay", "VoiceChatToggle", "+voicechat", NoFallback),
    a("TacticalMsg", "RadioAnswer", "radio_answer", NoFallback),
    a("ReportMsg", "RadioReport", "radio_report", NoFallback),
    a("HUDOnOff", "HUDOnOff", "hudonoff", IfFree("##201")),
    a("VideoCapture", "VideoCapture", "record", IfFree("##209")),
    a("Help", "HelpScreen", "helpscreen", NoFallback),
    a("radarmap_reduct", "DecRadarScale", "+decradarscale", NoFallback),
    a("radarmap_enlarge", "IncRadarScale", "+incradarscale", NoFallback),
    a("SuperSoldier", "UseMutationItem", "usemutationitem", IfFree("##56")),
    a("NPC_Shop", "CommandNPCShop", "commandnpcshop", IfFree("##34")),
    a("NPC_Order", "CommandNPCOrder", "commandnpcorder", IfFree("##35")),
    a("Use_Perk", "CommandUsePerk", "commandusepower", IfFree("##41")),
    a("QuestState", "QuestState", "queststate", IfFree("##36")),
    hidden("NightVision", "nightvision", NoFallback),
    hidden("Duck", "+duck", NoFallback),
    hidden("TeamChat", "chat_team", Always("##211")),
    hidden("BroadCastChat", "chat_all", NoFallback),
];

/// Commands in `ACTIONS` that nothing registers yet (their features
/// aren't built): the console reports them unknown when pressed.
pub const COMMANDS_UNBUILT: &[&str] = &[
    "record",
    "usemutationitem",
    "commandnpcshop",
    "commandnpcorder",
    "commandusepower",
    "queststate",
    "nightvision",
];

/// Keys 1 to 6 also run these (menu-line picks of the radio and emote
/// menus, K1's fixed keys).
pub const CHOOSE: [(&str, &str); 6] = [
    ("1", "choose_1"),
    ("2", "choose_2"),
    ("3", "choose_3"),
    ("4", "choose_4"),
    ("5", "choose_5"),
    ("6", "choose_6"),
];

/// The index of an action by its profile name (any case).
pub fn index_of(name: &str) -> Option<usize> {
    ACTIONS.iter().position(|a| a.name.eq_ignore_ascii_case(name))
}

/// An action's row control (K1) by layout name.
pub fn by_row(row: &str) -> Option<usize> {
    ACTIONS.iter().position(|a| a.row.is_some_and(|r| r.eq_ignore_ascii_case(row)))
}

/// One action's binding: a token (or nothing, empty) on each device.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Binding {
    pub kbd: String,
    pub mouse: String,
    pub joy: String,
}

/// The device slots of a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Keyboard,
    Mouse,
    Joystick,
}

impl Binding {
    /// Parse `"kbd@mouse@joystick"`: each part a token or a single space.
    pub fn parse(s: &str) -> Self {
        let mut p = s.split('@').map(|x| x.trim().to_string());
        Self {
            kbd: p.next().unwrap_or_default(),
            mouse: p.next().unwrap_or_default(),
            joy: p.next().unwrap_or_default(),
        }
    }

    /// The stored text: three slots, nothing as a single space.
    pub fn text(&self) -> String {
        let s = |x: &str| if x.is_empty() { " ".to_string() } else { x.to_string() };
        format!("{}@{}@{}", s(&self.kbd), s(&self.mouse), s(&self.joy))
    }

    pub fn slot(&self, slot: Slot) -> &str {
        match slot {
            Slot::Keyboard => &self.kbd,
            Slot::Mouse => &self.mouse,
            Slot::Joystick => &self.joy,
        }
    }

    pub fn slot_mut(&mut self, slot: Slot) -> &mut String {
        match slot {
            Slot::Keyboard => &mut self.kbd,
            Slot::Mouse => &mut self.mouse,
            Slot::Joystick => &mut self.joy,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.kbd.is_empty() && self.mouse.is_empty() && self.joy.is_empty()
    }

    /// The list's text for the row (K2): the keyboard key, ", " and the
    /// mouse button, ", " and the joystick control, skipping empty slots;
    /// `N/A` when there is none.
    pub fn row_text(&self, names: &KeyNames) -> String {
        let mut parts = Vec::new();
        if !self.kbd.is_empty() {
            parts.push(names.keyboard_text(&self.kbd));
        }
        if !self.mouse.is_empty() {
            parts.push(names.mouse_text(&self.mouse));
        }
        if !self.joy.is_empty() {
            parts.push(self.joy.clone());
        }
        if parts.is_empty() { "N/A".to_string() } else { parts.join(", ") }
    }
}

/// Every action's binding, in `ACTIONS` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingTable {
    pub entries: Vec<Binding>,
}

impl Default for BindingTable {
    fn default() -> Self {
        Self {
            entries: vec![Binding::default(); ACTIONS.len()],
        }
    }
}

impl BindingTable {
    /// The binding of an action by profile name.
    pub fn get(&self, name: &str) -> Option<&Binding> {
        index_of(name).map(|i| &self.entries[i])
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Binding> {
        index_of(name).map(|i| &mut self.entries[i])
    }

    /// The actions that hold `token` in `slot`, in table order.
    pub fn holders(&self, slot: Slot, token: &str) -> Vec<usize> {
        if token.is_empty() {
            return Vec::new();
        }
        (0..ACTIONS.len()).filter(|&i| self.entries[i].slot(slot) == token).collect()
    }

    /// Take `token` away from every action but `except` in `slot`.
    pub fn steal(&mut self, slot: Slot, token: &str, except: usize) {
        for i in 0..ACTIONS.len() {
            if i != except && self.entries[i].slot(slot) == token {
                self.entries[i].slot_mut(slot).clear();
            }
        }
    }

    /// Write every action's binding to the profile's `[Bindings]`
    /// (quoted, three slots); other entries of the section stay.
    pub fn store(&self, profile: &mut Profile) {
        for (action, b) in ACTIONS.iter().zip(&self.entries) {
            profile.set("Bindings", action.name, &crate::profile::quote(&b.text()));
        }
    }
}

/// Why loading a section stopped.
enum Stop {
    /// A "re-filled only if free" fallback met a taken key and the
    /// section is to be replaced by the default's (K4).
    Replace,
}

/// K4 over one profile's `[Bindings]`.
fn try_load(profile: &Profile, loading_defaults: bool) -> Result<BindingTable, Stop> {
    let mut table = BindingTable::default();
    for (i, action) in ACTIONS.iter().enumerate() {
        let entry = profile.string("Bindings", action.name);
        let mut b = entry.as_deref().map(Binding::parse).unwrap_or_default();
        if b.is_empty() {
            match action.fallback {
                NoFallback => {}
                Always(token) => b.kbd = token.to_string(),
                IfFree(token) => {
                    let free = !table.entries[..i].iter().any(|e| e.kbd == token);
                    if free {
                        b.kbd = token.to_string();
                    } else if entry.as_deref().is_none_or(str::is_empty) || loading_defaults {
                        return Err(Stop::Replace);
                    }
                }
            }
        }
        table.entries[i] = b;
    }
    Ok(table)
}

/// Load the table from a profile (K4). A "re-filled only if free" action
/// whose key is taken, with its entry absent or an empty string (or while
/// defaults load), replaces the whole `[Bindings]` by the default
/// profile's and loads again from there; a default that stops again
/// leaves the rest as the default has it.
pub fn load(profile: &Profile, default: &Profile, loading_defaults: bool) -> BindingTable {
    match try_load(profile, loading_defaults) {
        Ok(t) => t,
        Err(Stop::Replace) => {
            let mut replaced = profile.clone();
            replaced.replace_section("Bindings", default);
            try_load(&replaced, true).unwrap_or_else(|_| {
                // The default itself cannot fill a key: take it as it is.
                let mut t = BindingTable::default();
                for (i, action) in ACTIONS.iter().enumerate() {
                    if let Some(s) = default.string("Bindings", action.name) {
                        t.entries[i] = Binding::parse(&s);
                    }
                }
                t
            })
        }
    }
}

// ---------------------------------------------------------------------------
// K3: capturing a key

/// One raw input event the screen records while a capture waits (up to
/// `MAX_EVENTS` per frame).
#[derive(Clone, Debug, PartialEq)]
pub enum RawEvent {
    /// A keyboard key, by scan code.
    Key { code: u16, down: bool },
    /// A mouse button (`##3` is the left one).
    Button { code: u16, down: bool },
    /// The wheel, up or down (a notch is a key-down).
    Wheel { up: bool },
    /// Mouse axis motion: counts as an event, never decides.
    MouseMove,
    /// A joystick axis moving: counts as an event, never decides.
    JoyAxis,
    /// A joystick control going down or up, by its name.
    JoyButton { name: String, down: bool },
}

/// Events recorded per frame.
pub const MAX_EVENTS: usize = 8;

/// A row waiting for input (K3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    /// The action being bound.
    pub action: usize,
    /// The first event has come and erased the action's bindings.
    pub cleared: bool,
}

impl Capture {
    pub fn start(action: usize) -> Self {
        Self { action, cleared: false }
    }

    /// One frame's events (the first `MAX_EVENTS`). Returns whether the
    /// capture ended. `taken` says whether the engine already binds a
    /// keyboard key (by token) to something the options don't manage.
    pub fn frame(&mut self, events: &[RawEvent], table: &mut BindingTable, taken: &dyn Fn(Slot, &str) -> bool) -> bool {
        let events = &events[..events.len().min(MAX_EVENTS)];
        if events.is_empty() {
            return false;
        }
        if !self.cleared {
            self.cleared = true;
            table.entries[self.action] = Binding::default();
        }
        for e in events {
            let (slot, token) = match e {
                RawEvent::Wheel { up } => {
                    let token = if *up { WHEEL_UP } else { WHEEL_DOWN };
                    // Taken away from any other action, then given.
                    table.steal(Slot::Mouse, token, self.action);
                    table.entries[self.action].mouse = token.to_string();
                    return true;
                }
                RawEvent::Key { code, down: true } => (Slot::Keyboard, keys::token(*code)),
                RawEvent::Button { code, down: true } => (Slot::Mouse, keys::token(*code)),
                RawEvent::JoyButton { name, down: true } => (Slot::Joystick, name.clone()),
                // Key-up events, axis motion: skipped.
                _ => continue,
            };
            if remappable(slot, &token, taken) {
                table.steal(slot, &token, self.action);
                *table.entries[self.action].slot_mut(slot) = token;
            }
            return true;
        }
        false
    }
}

/// K3's remappable rule: not Esc, Pause or F8, and not a key the engine
/// binds to something the options don't manage.
pub fn remappable(slot: Slot, token: &str, taken: &dyn Fn(Slot, &str) -> bool) -> bool {
    !(slot == Slot::Keyboard && RESERVED.contains(&token)) && !taken(slot, token)
}

// ---------------------------------------------------------------------------
// The engine's side

/// The commands the options manage: every action's, and `choose_n`.
fn managed(part: &str) -> bool {
    let p = part.trim().to_lowercase();
    ACTIONS.iter().any(|a| a.command == p) || CHOOSE.iter().any(|(_, c)| *c == p)
}

/// Whether a console bind line holds any command the options manage.
fn has_managed(line: &str) -> bool {
    line.split(';').any(managed)
}

/// Whether the console binds `key` (Chimera's name) to something the
/// options don't manage (Tab's scoreboard, the debug keys): K3's rule.
pub fn engine_takes(console: &Console, key: &str) -> bool {
    console.binds.get(key).is_some_and(|line| !has_managed(line))
}

/// The key (Chimera's name) of a token in a slot; None for a joystick
/// control or an unknown token.
pub fn key_name(slot: Slot, token: &str) -> Option<&'static str> {
    match slot {
        Slot::Keyboard => keys::keyboard_name(token),
        Slot::Mouse => keys::mouse_name(token),
        Slot::Joystick => None,
    }
}

/// Make the console's binds those of `table`: the binds that run the
/// options' commands are dropped, then each action's keys run its
/// command (a key two actions hold runs both, in table order), and keys 1
/// to 6 also run their `choose_n`.
pub fn apply(console: &mut Console, table: &BindingTable) {
    console.binds.retain(|_, line| !has_managed(line));
    let mut lines: std::collections::BTreeMap<&'static str, Vec<&'static str>> = Default::default();
    for (action, b) in ACTIONS.iter().zip(&table.entries) {
        for (slot, token) in [(Slot::Keyboard, &b.kbd), (Slot::Mouse, &b.mouse)] {
            if let Some(key) = key_name(slot, token) {
                let v = lines.entry(key).or_default();
                if !v.contains(&action.command) {
                    v.push(action.command);
                }
            }
        }
    }
    for (key, choose) in CHOOSE {
        let v = lines.entry(key).or_default();
        v.push(choose);
    }
    for (key, commands) in lines {
        // The engine's own bind on the key (a default of Chimera's) gives
        // way to the player's.
        console.binds.insert(key.to_string(), commands.join("; "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_round_trips() {
        let b = Binding::parse("##17@ @ ");
        assert_eq!((b.kbd.as_str(), b.mouse.as_str(), b.joy.as_str()), ("##17", "", ""));
        assert_eq!(b.text(), "##17@ @ ");
        assert_eq!(Binding::parse(" @##3@ ").text(), " @##3@ ");
        assert_eq!(Binding::default().text(), " @ @ ");
    }

    #[test]
    fn the_table_has_the_spec_rows() {
        assert_eq!(ACTIONS.iter().filter(|a| a.row.is_some()).count(), 38);
        assert_eq!(ACTIONS.len(), 42);
        assert!(by_row("Zoomin").is_some());
        assert_eq!(index_of("prev_weapon"), index_of("Prev_Weapon"));
    }
}
