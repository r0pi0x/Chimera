//! CS:S's automatic help (`cl_autohelp`): short hints to a human player
//! the first time something happens to it ("you have spotted an enemy",
//! "press buy to purchase", ...), in the HUD's hint panel with the game's
//! hint sound. Which hint shows on what is data
//! (games/cs_source/data/hints.toml, `HintRules`: the game's localization
//! token, the trigger's name, a team, a delay; the sound and the gap
//! between hints); the triggers are hooks registered by name
//! (`HintAppExt::add_hint_triggers`: systems writing `HintTrigger`), so a
//! game or minigame adds its own hints and triggers without touching this
//! module (`HintAppExt::add_hint_data`).
//!
//! The rules run where the game is simulated (single player, a network
//! server: `core::authoritative`): `deliver` turns triggers into
//! `AutoHint`s to one character each, at most once per player per map,
//! only to humans (`LocalPlayer`, `RemotePlayer`; never bots), only while
//! that player's `cl_autohelp` (archived, sent as its userinfo,
//! `core::UserInfo`; on when unset) is on, and at least `gap` seconds
//! apart (a later one waits its turn). The local player's are shown by
//! the front end (`ui::hints`: the text from the install's
//! `resource/cstrike_english.txt`, `client::chat::Hint`, the sound); a
//! remote player's go to its client (`game::net`, `HintNews`).
//!
//! Known shortcuts (docs/tech-debt.md): every trigger is ours, guessed
//! from each hint's wording (CS:S's logic isn't public and no spec covers
//! it): standing in a buy zone while buying is open (`buy_zone`); an
//! enemy or a teammate first inside a 90 degree view cone with a clear
//! line, smoke ignored (`spotted_enemy`, `spotted_friend`, checked every
//! 8 ticks); killing an enemy, hurting or killing a teammate
//! (`killed_enemy`, `hurt_teammate`, `killed_teammate`); being given the
//! bomb at a round's start (`got_bomb`); carrying it inside a bomb target
//! (`bomb_target`); the gun in hand empty with no spare rounds
//! (`out_of_ammo`); night vision bought (`night_vision`); alive on a
//! hostage map (`hostage_map`, by team); within use reach of a hostage
//! nobody leads (`near_hostage`, by team); a hostage starting to follow
//! (`hostage_follows`); inside a rescue zone (`rescue_zone`); killing a
//! hostage (`killed_hostage`, 4 s later so the HUD's own message shows
//! first). The general rules are guesses too: once per player per map
//! (not per game or per round), off with `cl_autohelp 0` (a hint not shown
//! then isn't counted as shown), a 5 s gap (hints queue), humans only.
//! The game's hints without a trigger: the VIP and escape ones (no such
//! maps), `use_hostage_to_stop_him` (the follow hint says it too),
//! `cannot_play_because_tk` (`mp_tkpunish` kills instead),
//! `removed_for_next_hostage_killed` (always shown as the penalty warning,
//! `ui::objectives_hud`). Why it's OK for now: capture item in
//! docs/plans/active/windows-captures.md, sitting 1 (which hints appear
//! when on a fresh profile).

use std::collections::{BTreeSet, VecDeque};

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::ScheduleSystem, prelude::*};
use serde::Deserialize;

use crate::{
    console::{Console, resource_cvar},
    core::{
        Bot, Dead, DamageTaken, Died, Health, Intent, LocalPlayer, MapLoads, MovementState, Npc, RemotePlayer,
        SimClock, Spectating, Team, UserInfo, authoritative,
    },
    objectives::{
        MapKind, MapObjectives, ObjectiveEvent,
        bomb::{carried_bomb, hull},
        hostages::Hostage,
        use_search::USE_RADIUS,
    },
    weapon::{
        Inventory, Magazine,
        economy::{BuyWindow, NightVision, cannot_buy_here},
    },
};

/// The userinfo key and cvar of the setting (CS:S's Multiplayer >
/// Advanced "Auto-help", default 1).
pub const AUTOHELP_KEY: &str = "cl_autohelp";

/// CS:S's hints (built in).
const HINTS_DATA: (&str, chimera_core::game_data::GameData) = ("hints.toml", chimera_core::game_data!("data/hints.toml"));

/// Ticks between the checks of where players stand and what they see.
const CHECK_TICKS: u64 = 8;
/// Half the view cone a character must be in to count as spotted (ours:
/// a 90 degree cone, CS:S's default field of view).
const SPOT_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HintRules>()
        .init_resource::<HintTriggers>()
        .init_resource::<AutoHelp>()
        .add_message::<HintTrigger>()
        .add_message::<AutoHint>()
        .add_message::<ObjectiveEvent>()
        .add_message::<Died>()
        .add_message::<DamageTaken>()
        .configure_sets(
            FixedPostUpdate,
            (HintSystems::Triggers, HintSystems::Deliver)
                .chain()
                .run_if(authoritative)
                .run_if(crate::objectives::plays),
        )
        .add_systems(
            FixedPostUpdate,
            keep_history.before(HintSystems::Triggers).run_if(crate::objectives::plays),
        )
        .add_systems(FixedPostUpdate, deliver.in_set(HintSystems::Deliver));
    resource_cvar::<AutoHelp, u8>(
        app,
        AUTOHELP_KEY,
        "1: hints for new players the first time something happens (Multiplayer > Advanced \"Auto-help\"; sent to the server as userinfo).",
        |a| &mut a.0,
    );
    {
        let mut console = app.world_mut().resource_mut::<Console>();
        console.archive(AUTOHELP_KEY);
        console.userinfo(AUTOHELP_KEY);
    }
    app.add_hint_triggers(&["buy_zone"], buy_zone)
        .add_hint_triggers(&["spotted_enemy", "spotted_friend"], spotting)
        .add_hint_triggers(&["killed_enemy", "killed_teammate"], kills)
        .add_hint_triggers(&["hurt_teammate"], team_damage)
        .add_hint_triggers(&["got_bomb", "hostage_follows", "killed_hostage"], objective_events)
        .add_hint_triggers(&["bomb_target", "hostage_map", "rescue_zone"], zones)
        .add_hint_triggers(&["near_hostage"], near_hostage)
        .add_hint_triggers(&["out_of_ammo"], out_of_ammo)
        .add_hint_triggers(&["night_vision"], night_vision)
        .add_hint_data(HINTS_DATA.0, HINTS_DATA.1.text());
}

/// `cl_autohelp` (CS:S's default 1).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct AutoHelp(pub u8);

impl Default for AutoHelp {
    fn default() -> Self {
        Self(1)
    }
}

/// The hints' order: the triggers, then what they show.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HintSystems {
    Triggers,
    Deliver,
}

/// Something a hint may be shown for happened to `who` (a trigger's
/// name, as the data names it).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HintTrigger {
    pub who: Entity,
    pub when: &'static str,
}

/// Show hint `token` (the game's localization token) to `to`'s player:
/// passed the rules (once, `cl_autohelp`, the gap). A network client
/// writes the server's for its own player (`game::net`).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct AutoHint {
    pub to: Entity,
    pub token: String,
}

/// One hint of the data: its token, the trigger it shows on, only for
/// this team (`core::Team` number), this many seconds after.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HintRule {
    pub token: String,
    pub when: String,
    #[serde(default)]
    pub team: Option<u8>,
    #[serde(default)]
    pub delay: f32,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Settings {
    sound: Option<String>,
    gap: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    hint: Vec<HintRule>,
}

/// The hints (the data documents loaded so far).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct HintRules {
    /// The sound entry a hint plays.
    pub sound: Option<String>,
    /// Seconds at least between two hints to one player.
    pub gap: f32,
    pub hints: Vec<HintRule>,
}

impl HintRules {
    /// Add a document (`name` for errors): its hints after those there
    /// (one with a token already there replaces it), its settings over
    /// them. Every trigger it names must be one of `known`.
    pub fn load(&mut self, name: &str, text: &str, known: &BTreeSet<&'static str>) -> Result<(), String> {
        let doc: Document = super::module::parse_settings(name, text)?;
        for h in &doc.hint {
            if !known.contains(h.when.as_str()) {
                return Err(format!("{name}: hint {}: no trigger named \"{}\"", h.token, h.when));
            }
            if h.token.trim().is_empty() {
                return Err(format!("{name}: a hint without a token"));
            }
        }
        if let Some(s) = doc.settings.sound {
            self.sound = Some(s);
        }
        if let Some(g) = doc.settings.gap {
            self.gap = g.max(0.0);
        }
        for h in doc.hint {
            match self.hints.iter_mut().find(|o| o.token == h.token && o.when == h.when) {
                Some(o) => *o = h,
                None => self.hints.push(h),
            }
        }
        Ok(())
    }
}

/// CS:S's hint sound (its data's), for the sound loader.
pub fn sound_entry() -> Option<String> {
    let doc: Document = super::module::parse_settings(HINTS_DATA.0, HINTS_DATA.1.text()).ok()?;
    doc.settings.sound
}

/// The trigger names registered.
#[derive(Resource, Clone, Debug, Default)]
pub struct HintTriggers(pub BTreeSet<&'static str>);

/// Registering triggers and hint data.
pub trait HintAppExt {
    /// Triggers named `names`: `system` (in `FixedPostUpdate`, where the
    /// game is simulated) writes `HintTrigger`s with those names.
    fn add_hint_triggers<M>(
        &mut self,
        names: &[&'static str],
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;
    /// A hints document (TOML: `[settings]` sound and gap, `[[hint]]`
    /// token, when, team, delay), after the triggers it names. A bad
    /// document is an error at start.
    fn add_hint_data(&mut self, name: &str, text: &str) -> &mut Self;
}

impl HintAppExt for App {
    fn add_hint_triggers<M>(
        &mut self,
        names: &[&'static str],
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.init_resource::<HintTriggers>();
        self.world_mut().resource_mut::<HintTriggers>().0.extend(names);
        self.add_systems(FixedPostUpdate, system.in_set(HintSystems::Triggers))
    }

    fn add_hint_data(&mut self, name: &str, text: &str) -> &mut Self {
        let known = self.world().resource::<HintTriggers>().0.clone();
        let mut rules = self.world_mut().resource_mut::<HintRules>();
        if let Err(e) = rules.load(name, text, &known) {
            panic!("hint data {e}");
        }
        self
    }
}

/// What a player was shown on this map and what waits.
#[derive(Component, Clone, Debug, Default)]
pub struct HintHistory {
    /// Tokens shown (or waiting), in order.
    pub shown: Vec<String>,
    /// The map they were shown on (`MapLoads`).
    map: u32,
    /// Waiting: token, not before (`SimClock::now`).
    queue: VecDeque<(String, f64)>,
    /// No hint before this.
    next: f64,
}

/// Humans: the local player and remote ones, never bots.
type Human = (Or<(With<LocalPlayer>, With<RemotePlayer>)>, Without<Bot>, Without<Npc>);

/// Alive, in the game.
fn alive(health: &Health, dead: bool, spectating: bool) -> bool {
    health.current > 0.0 && !dead && !spectating
}

/// Every human gets a history.
fn keep_history(people: Query<Entity, (Human, Without<HintHistory>)>, loads: Option<Res<MapLoads>>, mut commands: Commands) {
    let map = loads.map_or(0, |l| l.0);
    for e in &people {
        commands.entity(e).insert(HintHistory {
            map,
            ..default()
        });
    }
}

/// Triggers into hints: once per player per map, with its `cl_autohelp`
/// on, its team's; then each player's next one when its turn comes.
fn deliver(
    mut triggers: MessageReader<HintTrigger>,
    rules: Res<HintRules>,
    clock: Res<SimClock>,
    loads: Option<Res<MapLoads>>,
    mut people: Query<(Entity, &mut HintHistory, Option<&Team>, Option<&UserInfo>), Human>,
    mut out: MessageWriter<AutoHint>,
) {
    let now = clock.now;
    let map = loads.map_or(0, |l| l.0);
    for (_, mut h, ..) in &mut people {
        if h.map != map {
            *h = HintHistory {
                map,
                ..default()
            };
        }
    }
    for t in triggers.read() {
        let Ok((_, mut history, team, info)) = people.get_mut(t.who) else { continue };
        if info.and_then(|i| i.flag(AUTOHELP_KEY)) == Some(false) {
            continue;
        }
        for rule in rules.hints.iter().filter(|r| r.when == t.when) {
            if rule.team.is_some_and(|want| team.map(|t| t.0) != Some(want)) || history.shown.contains(&rule.token) {
                continue;
            }
            history.shown.push(rule.token.clone());
            history.queue.push_back((rule.token.clone(), now + rule.delay as f64));
        }
    }
    for (e, mut history, ..) in &mut people {
        if now < history.next {
            continue;
        }
        let Some(i) = history.queue.iter().position(|(_, at)| *at <= now) else { continue };
        let (token, _) = history.queue.remove(i).expect("a waiting hint");
        history.next = now + rules.gap as f64;
        out.write(AutoHint { to: e, token });
    }
}

// --- Triggers (all ours: rules::hints module docs)

/// Every `CHECK_TICKS` ticks.
fn check_tick(world: &World) -> bool {
    world.get_resource::<SimClock>().is_none_or(|c| c.tick % CHECK_TICKS == 0)
}

/// The living humans.
fn living_humans(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<(Entity, &Health, Has<Dead>, Has<Spectating>), Human>()
        .iter(world)
        .filter(|(_, h, d, s)| alive(h, *d, *s))
        .map(|(e, ..)| e)
        .collect()
}

/// `buy_zone`: in one of its buy zones while buying is open.
fn buy_zone(world: &mut World) {
    if !check_tick(world) || world.get_resource::<BuyWindow>().is_some_and(|b| b.0.is_err()) {
        return;
    }
    for e in living_humans(world) {
        if cannot_buy_here(world, e).is_none() {
            world.write_message(HintTrigger { who: e, when: "buy_zone" });
        }
    }
}

/// `spotted_enemy`, `spotted_friend`: another living player inside the
/// view cone with a clear line from the eye.
fn spotting(
    clock: Res<SimClock>,
    spatial: SpatialQuery,
    viewers: Query<(Entity, &Transform, &Intent, Option<&MovementState>, Option<&Team>, &Health, Has<Dead>, Has<Spectating>), Human>,
    others: Query<(Entity, &Transform, Option<&MovementState>, &Team, &Health, Has<Dead>), Without<Npc>>,
    mut out: MessageWriter<HintTrigger>,
) {
    if clock.tick % CHECK_TICKS != 0 {
        return;
    }
    let eye_of = |t: &Transform, s: Option<&MovementState>| t.translation + s.map_or(Vec3::Y * 0.6, |s| s.eye_offset);
    for (me, t, intent, state, team, health, dead, spec) in &viewers {
        let Some(team) = team.filter(|_| alive(health, dead, spec)) else { continue };
        let eye = eye_of(t, state);
        let forward = Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0) * Vec3::NEG_Z;
        let (mut enemy, mut friend) = (false, false);
        for (e, ot, os, oteam, oh, odead) in &others {
            if e == me || oh.current <= 0.0 || odead {
                continue;
            }
            let friendly = oteam == team;
            if (friendly && friend) || (!friendly && enemy) {
                continue;
            }
            let to = eye_of(ot, os) - eye;
            let Ok(dir) = Dir3::new(to) else { continue };
            if dir.dot(forward) < SPOT_HALF_ANGLE.cos() {
                continue;
            }
            let filter = SpatialQueryFilter::from_excluded_entities([me, e]).with_mask(crate::core::SOLID_LAYERS);
            if spatial.cast_ray(eye, dir, to.length(), true, &filter).is_some() {
                continue;
            }
            if friendly {
                friend = true;
            } else {
                enemy = true;
            }
        }
        if enemy {
            out.write(HintTrigger { who: me, when: "spotted_enemy" });
        }
        if friend {
            out.write(HintTrigger { who: me, when: "spotted_friend" });
        }
    }
}

/// `killed_enemy`, `killed_teammate`: a player died by a human's hand.
fn kills(mut died: MessageReader<Died>, teams: Query<&Team>, mut out: MessageWriter<HintTrigger>) {
    for d in died.read() {
        let Some(by) = d.attacker.filter(|a| *a != d.entity) else { continue };
        let (Ok(theirs), Ok(mine)) = (teams.get(d.entity), teams.get(by)) else { continue };
        let when = if theirs == mine { "killed_teammate" } else { "killed_enemy" };
        out.write(HintTrigger { who: by, when });
    }
}

/// `hurt_teammate`: damage that reached a teammate.
fn team_damage(mut taken: MessageReader<DamageTaken>, teams: Query<&Team>, mut out: MessageWriter<HintTrigger>) {
    for t in taken.read() {
        let target = t.damage.target;
        let Some(by) = t.damage.attacker.filter(|a| *a != target) else { continue };
        if let (Ok(theirs), Ok(mine)) = (teams.get(target), teams.get(by))
            && theirs == mine
        {
            out.write(HintTrigger { who: by, when: "hurt_teammate" });
        }
    }
}

/// `got_bomb`, `hostage_follows`, `killed_hostage`: from what the
/// objectives say happened.
fn objective_events(mut events: MessageReader<ObjectiveEvent>, mut out: MessageWriter<HintTrigger>) {
    for ev in events.read() {
        let (who, when) = match ev {
            ObjectiveEvent::GotBomb { who } => (*who, "got_bomb"),
            ObjectiveEvent::HostageFollows { leader, .. } => (*leader, "hostage_follows"),
            ObjectiveEvent::HostageKilled { attacker: Some(a), .. } => (*a, "killed_hostage"),
            _ => continue,
        };
        out.write(HintTrigger { who, when });
    }
}

/// `bomb_target` (carrying the bomb inside a target), `hostage_map`
/// (alive on a hostage map), `rescue_zone` (inside a rescue zone).
fn zones(world: &mut World) {
    if !check_tick(world) {
        return;
    }
    let Some(objectives) = world.get_resource::<MapObjectives>() else { return };
    let hostage_map = objectives.kind() == MapKind::Hostage;
    let mut fired = Vec::new();
    for e in living_humans(world) {
        let Some(t) = world.get::<Transform>(e) else { continue };
        let (lo, hi) = hull(t, world.get::<MovementState>(e));
        let objectives = world.resource::<MapObjectives>();
        if hostage_map {
            fired.push((e, "hostage_map"));
        }
        if objectives.rescue_zones.iter().any(|z| z.overlaps(lo, hi)) {
            fired.push((e, "rescue_zone"));
        }
        if objectives.bomb_target_at(lo, hi).is_some() && carried_bomb(world, e).is_some() {
            fired.push((e, "bomb_target"));
        }
    }
    for (who, when) in fired {
        world.write_message(HintTrigger { who, when });
    }
}

/// `near_hostage`: within use reach of a living hostage nobody leads.
fn near_hostage(
    clock: Res<SimClock>,
    people: Query<(Entity, &Transform, &Health, Has<Dead>, Has<Spectating>), Human>,
    hostages: Query<(&Transform, &Hostage, &Health)>,
    mut out: MessageWriter<HintTrigger>,
) {
    if clock.tick % CHECK_TICKS != 0 || hostages.is_empty() {
        return;
    }
    for (e, t, h, dead, spec) in &people {
        if !alive(h, dead, spec) {
            continue;
        }
        let near = hostages.iter().any(|(ht, hostage, hh)| {
            hh.current > 0.0 && hostage.leader.is_none() && ht.translation.distance(t.translation) <= USE_RADIUS
        });
        if near {
            out.write(HintTrigger { who: e, when: "near_hostage" });
        }
    }
}

/// `out_of_ammo`: the gun in hand has an empty clip and no spare rounds.
fn out_of_ammo(
    people: Query<(Entity, &Inventory, &Health), Human>,
    mags: Query<&Magazine>,
    mut out: MessageWriter<HintTrigger>,
) {
    for (e, inv, h) in &people {
        if h.current <= 0.0 {
            continue;
        }
        if let Some(m) = inv.active.and_then(|w| mags.get(w).ok())
            && m.size > 0
            && m.clip == 0
            && m.reserve == 0
        {
            out.write(HintTrigger { who: e, when: "out_of_ammo" });
        }
    }
}

/// `night_vision`: goggles just bought (or given).
fn night_vision(people: Query<Entity, (Human, Added<NightVision>)>, mut out: MessageWriter<HintTrigger>) {
    for e in &people {
        out.write(HintTrigger { who: e, when: "night_vision" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> BTreeSet<&'static str> {
        let mut app = App::new();
        app.add_plugins(crate::console::ConsolePlugin);
        plugin(&mut app);
        app.world().resource::<HintTriggers>().0.clone()
    }

    #[test]
    fn the_games_hints_load_and_name_registered_triggers() {
        let mut rules = HintRules::default();
        rules.load(HINTS_DATA.0, HINTS_DATA.1.text(), &known()).unwrap();
        assert_eq!(rules.sound.as_deref(), Some("Hud.Hint"));
        assert_eq!(sound_entry().as_deref(), Some("Hud.Hint"));
        assert!(rules.gap > 0.0);
        assert!(rules.hints.len() >= 17, "{}", rules.hints.len());
        assert!(rules.hints.iter().all(|h| h.token.starts_with("Cstrike_TitlesTXT_Hint_")));
    }

    #[test]
    fn unknown_triggers_and_keys_are_errors() {
        let mut rules = HintRules::default();
        let e = rules
            .load("x.toml", "[[hint]]\ntoken = \"A\"\nwhen = \"nope\"\n", &known())
            .unwrap_err();
        assert!(e.contains("x.toml") && e.contains("nope"), "{e}");
        assert!(rules.load("x.toml", "[[hint]]\ntoken = \"A\"\nwhen = \"buy_zone\"\nfoo = 1\n", &known()).is_err());
        // A later document replaces a hint and adds others.
        rules.load("a", "[[hint]]\ntoken = \"A\"\nwhen = \"buy_zone\"\n", &known()).unwrap();
        rules
            .load("b", "[settings]\ngap = 1.0\n[[hint]]\ntoken = \"A\"\nwhen = \"buy_zone\"\ndelay = 2.0\n[[hint]]\ntoken = \"B\"\nwhen = \"got_bomb\"\n", &known())
            .unwrap();
        assert_eq!(rules.gap, 1.0);
        assert_eq!(rules.hints.len(), 2);
        assert_eq!(rules.hints[0].delay, 2.0);
    }
}
