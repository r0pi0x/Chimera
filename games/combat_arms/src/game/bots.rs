//! Combat Arms' bots (specs/combat_arms/bots.md): the computer soldiers of
//! Bot Deathmatch, as characters whose `Intent` comes from a brain (like
//! CS:S's bots), moving with Combat Arms' movement and firing its weapons.
//!
//! - `roster`: adding bots (`ca_bot_add`, the `MaxPlayer` quota), their
//!   profile and item set by the roster's dice ("Choosing the bots": the
//!   profile probe, the item set's slots by the draw's digits, the lists
//!   padded to different lengths), the team opposite the human (10), the
//!   kit given every life (`weapon::OwnKit`) with the gun type's ammunition
//!   multiplier, the ranged and thrown reach fixed at spawn (8.2, 8.5).
//!   They come in as players do (`rules::flow::enter_game`) and respawn by
//!   the mode's rules (the respawn request after 7 s, `rules::spawns`).
//! - `data`: data/bots.toml, each attribute template's numbers and the
//!   per-weapon AI rows by gun type (`BotLevel`: 0 Easy, 1 Normal, 2
//!   Veteran, 3 training).
//! - `math`: the formulas (rolls, the hit/miss cycle, aim points, the
//!   movement inaccuracy, the reaction delay, camping, weapon choice, the
//!   throw, attack moves, patrol links).
//! - `map_ai`: the map's own AI volumes and patrol nodes (`game::world`
//!   passes the AI objects through as map entities), the navigation areas
//!   and patrol graph where a map has them.
//! - `nav`: the volume path search on the map's navigation areas: its AI
//!   volumes (`map_ai`), else generated from the map's collision
//!   (`chimera_world::map::nav_gen`; `ensure_nav`).
//! - `brain`: the senses (5), goals and states (2: patrol, investigate,
//!   chase, attack, dodge), firing (7, 8), movement in a fight (9).
//!
//! Objectives (bots_objectives.md): no AI in Combat Arms plays one, so
//! bots take no flag, bomb, intel, seize trigger, bullion or VIP role
//! (`rules::objectives::takers` leaves them out) and no objective event
//! reaches their senses; they fight in every mode. Hired Guns'
//! mercenaries are bots too (`MercMind`: the `Squad` goal set, follow and
//! attack only; `rules::mercs`), but not players.
//!
//! Bots run on the server only, their senses and goals on the server's
//! 20 Hz frames (`grenades::SERVER_FRAME`, the dedicated server's rate:
//! Open question 1), their aim, movement keys and trigger every tick.
//!
//! The follow-up answers (bots.md Open questions 3, 6, 11, 14, 16-24):
//! a bot's rounds wait its fire animation's interval by class
//! (`math::fire_interval_ms`); its rounds get the hit-zone factors on the
//! server's pose (`characters::CaPose::server_spheres`); jump-up volumes
//! link one way (`map_ai`); the sight's eye is the bot's hull x/z at its
//! `Head` node's height, its targets the hull centre then the `Head` node;
//! footsteps are posted at the keys of each character's leg animation
//! (running, sprinting and the jumps' landings: walking and crouching are
//! silent) and the footstep sense counts false stimulations
//! (`math::Sense`); the dodge tries its five directions in the spec's
//! order (`math::DODGE_STEPS`); the knife closes to its 130 u reach; a bot
//! is protected 3 s from every damage and respawns 7 s after its death
//! (`rules::spawns`); bots chat on their kills of players, deaths,
//! respawns, battles, searches and grenade dodges (`chat`). Ally stimuli
//! stay off (Open question 14: confirmed).
//!
//! The 2026-10-22 corrections: a heard sound's investigation ends within
//! half the distance it started from (Correction 1); a sound is only
//! looked at when the life's alarm total is under the major threshold, 30 s
//! have passed since the last relaxed moment (the spawn counts), and it is
//! within 1000 u and in view (2); an investigation ignores calls and takes
//! only an equal or higher alarm (3); the grenade dodge goes onto a grenade
//! just outside the blast radius, kept for parity (4); chat events from
//! the brain, the profile's cooldown (5, 6); every life a fresh brain (7);
//! patrol stops and per-team start nodes (8); the throw as an action from
//! the hand (9); blind chase fire along the torso (10); the `Torso` aim
//! point and the line of fire from the right side (11). Mercenaries aim
//! like PvE AI (bots_objectives.md 3.3; `brain::aim_and_fire`).
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear, listed for its open questions):
//! - Open question 6 (the design stays ours): bots move and fire through
//!   the player's parts (the brain writes `Intent`), not at constant
//!   template speeds confined to AI volumes: they run at Combat Arms'
//!   movement speed (the template's run and jump speeds are data only),
//!   jump and fall as players, and nothing refuses a step off the volumes.
//!   Their shots go through the weapon frame, with `weapons::ExactAim`
//!   taking the spread cone, the recoil kick and extra pellets away (one
//!   ray per shot, Quirk 7), so each round goes where the brain aims; the
//!   weapon's own cycle binds where it is longer than the bot's interval
//!   (an M4A1 at 105 ms, not 100).
//! - Navigation: the map's AI volumes where it has them (`map_ai`, whose
//!   Known shortcuts say what of them isn't used: ladders, jumps, doors);
//!   elsewhere the areas are generated from the collision (0.5 m cells
//!   merged into rectangles, rises up to 0.8 m with a jump,
//!   `brain::nav_params`), where the 48 u link minimum, doors, ladders and
//!   teleport volumes don't apply. Patrol nodes: the map's `AINodePatrol`s
//!   where it has them, else ours (`nav::PatrolGraph`); either way with
//!   the spec's link choice, the 25 % stops at nodes and (on a map's own
//!   nodes) the start nodes `AI_Node_<A|B>_01..08` round robin per team
//!   (the bot game-mode file isn't read); no look-arounds while running
//!   (head only) and no guard nodes. A bot that makes no progress for
//!   1.5 s plans again and jumps (the spec has none: Open question 11,
//!   still open).
//! - Senses: the `Head` height is the standing pose's (not crouched or
//!   animated); the sight shortcut through areas is the floor-level
//!   straight walk; sight rays meet the gameplay layers
//!   (`core::SOLID_LAYERS`) but for see-through surfaces (Chimera's
//!   `SeeThrough`); a footstep's radius is 256 u on every surface (the
//!   surface's `MoveNoiseMod` isn't known to the bots). Grenade danger
//!   comes from grenades thrown by players (not bots) that touched
//!   something or rest, read from `grenades::CaGrenade`.
//! - Not there: placed devices (mines, claymores), so no mine drops while
//!   patrolling; targets are characters but for the auto turret (ours,
//!   `brain::turret_step`: a bot carrying one with none standing places
//!   one, a bot with a loaded gun shoots back at an enemy turret within
//!   1000 u in sight that is shooting at it); doors (and their
//!   retry); the "catch breath" stop lasts 1 s (its length is open: Open
//!   question 20); a roll is a fast sidestep; the knife is the player's
//!   knife with the bot's 130 u reach (`inventory::KnifeReach`; its swing
//!   trace and zones stay, where the spec deals the damage straight to
//!   the target); bodies; chat on the mine event and for network clients.
//! - Values the spec leaves open, with our fallbacks: the alarm levels
//!   (`[alarm]`: major 50, gunfire 75, footstep 20, call 1; Open question
//!   25); the sound is looked at only within 1000 u (the straight-walk look
//!   is limited the same, to match the expected cases); a bot always turns to
//!   a sound it only looks at; a call's answer faces the caller's yaw for
//!   a tick; the throw's key and length are the grenade's (`Throwable`) for
//!   both animations and the hand is a point beside and below the eye
//!   (Open question 27); the chasing torso turns up to 90 degrees of yaw
//!   and 1 rad of pitch from the run heading (28); the crouched `Torso` is
//!   the standing one's; a chasing bot's reload refills its clip at once.
//! - Profiles: the install's profile file isn't read: a bot is
//!   `AI_<Team>_<profile>`, its archetype the profile number mod 3, its
//!   item sets ours (data/bots.toml), the draw ρ our seeded dice (any
//!   uniform draw is parity, Open question 22). With no human, a bot joins
//!   the smaller team, so bots fight bots.
//!
//! The lift of CS:S's bot brain into Chimera (L16) is not done: Combat
//! Arms' brain shares no rule with CS:S's (no turn rate, a hit/miss cycle
//! instead of aim error, squared path costs over areas, no team plans), so
//! only the navigation mesh (already Chimera's) and the new mesh generator
//! are shared.

pub mod brain;
pub mod chat;
pub mod data;
pub mod map_ai;
pub mod math;
pub mod nav;
pub mod roster;

use bevy::prelude::*;

pub use data::{BotData, GunAi, Level, Template};
pub use roster::{add_bot, auto_team, kick};

use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::SimSet,
};
use math::{Dice, HitCycle, Tracking};

/// The settings: `MaxPlayer` (bots the server keeps), `BotLevel`.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BotSettings {
    /// `MaxPlayer`: the number of bots.
    pub quota: u32,
    /// Whether the quota was set (the server keeps it only then).
    pub quota_set: bool,
    /// `BotLevel`: 0 Easy, 1 Normal, 2 Veteran (3: training).
    pub level: u8,
    /// `ca_bot_stop`: 1 keeps every bot's movement keys off (they still
    /// see, aim and fire).
    pub stop: u8,
    /// `ca_bot_dont_shoot`: 1 keeps their fingers off the trigger.
    pub dont_shoot: u8,
    /// The roster's dice seed.
    pub seed: u64,
    /// How wide the generated navigation mesh's walker is (half, m), where
    /// an app's bots may wear a wider hull than Combat Arms' (None: its own,
    /// `brain::nav_params`).
    pub walker_radius: Option<f32>,
}

/// What a bot is doing (2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BotState {
    /// The idle behaviour: walking the patrol graph.
    #[default]
    Patrol,
    /// Going to a heard spot (or looking at it), until `until`: over on
    /// the first update within `radius` (m, half the straight-line
    /// distance at the start) of `at` (the sound, Correction 1). `level`
    /// is the alarm level of the stimulus followed and `id` its id
    /// (Correction 3); `face` the yaw to face on arrival (a call's).
    Investigate {
        spot: Vec3,
        until: f64,
        look_only: bool,
        at: Vec3,
        radius: f32,
        level: f32,
        id: u64,
        face: Option<f32>,
    },
    /// Pursuing the target, knowing where it is, until `until`; standing
    /// for breath until `breath`.
    Chase { until: f64, breath: f64 },
    /// Fighting the target since `since`.
    Attack { since: f64 },
    /// Moving to `to` (away from grenade `grenade`, or onto it when just
    /// outside its blast radius, 5.4 Correction 4), until the grenade is
    /// gone, the move ends, or `until`; `chase`: it was chasing when the
    /// dodge began.
    Dodge { to: Vec3, until: f64, grenade: Entity, chase: bool },
}

impl BotState {
    /// Patrolling or investigating: the notice rolls apply, the FOV is
    /// relaxed.
    pub fn relaxed(&self) -> bool {
        matches!(self, BotState::Patrol | BotState::Investigate { .. })
    }
}

/// A heard stimulus waiting for its reaction delay (5.1: counted in sense
/// passes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub at_pass: u64,
    pub heard: brain::Heard,
}

/// A throw in progress (8.5, Correction 9): the bot has switched to its
/// grenade and plays the throw animation; no round is fired until it is
/// over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    pub weapon: Entity,
    /// When the throw was decided (a switch that never completes ends it).
    pub begun: f64,
    /// When the grenade was drawn (the animation starts), once it is.
    pub started: Option<f64>,
    /// The close-range throw animation (the target nearer than 1000 u).
    pub near: bool,
    pub released: bool,
}

/// A Hired Guns mercenary's mind (bots_objectives.md 3; `rules::mercs`):
/// whom it follows and how, beside the bot brain's attack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MercMind {
    /// The hirer (its leader).
    pub leader: Entity,
    /// The follow point (feet, m); None or the world origin: the leader
    /// (Quirk 4).
    pub follow_point: Option<Vec3>,
    /// "Follow me": the follow goal outranks every attack (Quirk 3).
    pub follow_only: bool,
    /// Holding (within the arrive radius) rather than following.
    pub holding: bool,
    /// While holding: the leave delay's end (restarted while inside the
    /// leave radius).
    pub leave_at: f64,
    /// While following: the path's next re-plan.
    pub replan_at: f64,
    /// When an enemy was last seen (the attack goal's 2 s decay).
    pub seen_at: f64,
}

impl MercMind {
    pub fn new(leader: Entity) -> Self {
        Self {
            leader,
            follow_point: None,
            follow_only: false,
            holding: false,
            leave_at: 0.0,
            replan_at: 0.0,
            seen_at: f64::MIN,
        }
    }
}

/// A bot: its brain's state. Positions in metres (engine), the spec's
/// numbers in world units where named so.
#[derive(Component, Clone, Debug)]
#[require(crate::core::Bot)]
pub struct CaBot {
    pub profile: usize,
    pub template: Template,
    pub dice: Dice,
    /// The kit: mid primary, long primary, secondary, melee, grenade, mine.
    pub kit: roster::Kit,
    /// This life's kit has been set up (`roster::arm`).
    pub armed: bool,
    /// The ranged and thrown reach (u), fixed at the first spawn.
    pub reach_fixed: bool,
    pub ranged_reach: f32,
    pub thrown_reach: f32,

    pub state: BotState,
    pub target: Option<Entity>,
    /// The target's last sight result: visible, only the head.
    pub visible: bool,
    pub head_only: bool,
    pub tracking: Tracking,
    pub cycle: HitCycle,
    /// The current shot's aim offset from the base point (u): None until
    /// drawn for the next shot.
    pub aim_offset: Option<Vec3>,
    /// Exact aim for this shot (inside the full-accuracy zone).
    pub exact: bool,

    // Senses.
    /// Sense passes run.
    pub passes: u64,
    /// The sight cooldown ends (patrolling, 5.2).
    pub sight_cooldown: f64,
    /// The footstep sense's stimulation, the newest step it took.
    pub footsteps: math::Sense,
    /// Where the last heard footstep was.
    pub last_step_at: Option<Vec3>,
    /// The next round the fire animation allows (s, 8.3).
    pub next_shot: f64,
    pub last_step: u64,
    pub last_gunfire: u64,
    pub last_call: u64,
    pub pending: Option<Pending>,

    // Attack.
    /// When the attack goal last went off; when firing is allowed.
    pub attack_off: f64,
    pub fire_at: f64,
    /// While the target is hidden in an attack: when the attack ends.
    pub chase_timer: Option<f64>,
    pub held: Option<Entity>,
    pub burst_left: u32,
    pub pause_until: f64,
    pub last_clip: Option<u32>,
    /// Shots before a grenade (8.5).
    pub grenades: u32,
    pub next_move: f64,
    pub move_to: Option<Vec3>,
    pub move_until: f64,
    pub dodge_next: f64,
    /// Hit since the last frame (9.2's dodge check).
    pub dodge_hit: bool,
    pub crouch_until: f64,
    /// Let go of the trigger this tick (after a shot, for semi-automatic
    /// weapons).
    pub release: bool,

    // Grenade danger (5.4).
    pub danger: Option<(Entity, Vec3, f64)>,
    /// The throw in progress (8.5).
    pub throw: Option<Throw>,

    // Alarm (5.3, Corrections 2, 3): the running total of every sound
    // investigated this life; when the bot last returned to relaxed
    // awareness (its spawn counts; None: not spawned yet).
    pub alarm: f32,
    pub last_relaxed: Option<f64>,
    /// Chat events the brain raised, for `chat::chat`.
    pub chat: Vec<chat::Event>,
    /// A new life that has not taken its patrol start node (4).
    pub needs_start: bool,
    /// Standing at a patrol node to look around until then (4).
    pub patrol_wait: f64,
    /// A mercenary aims like PvE AI (bots_objectives.md 3.3 Correction 1):
    /// set by its tick.
    pub merc_aim: bool,
    /// A mercenary's aim point above its target's hull centre (u).
    pub merc_height: f32,

    // Navigation.
    pub route: Vec<Vec3>,
    pub route_goal: Option<Vec3>,
    pub patrol_from: Option<usize>,
    pub patrol_to: Option<usize>,
    /// The patrol node it last left (not taken back unless a dead end).
    pub patrol_came: Option<usize>,
    pub stuck_check: (f64, Vec3),
    pub jump: bool,
    /// A Hired Guns mercenary's mind (None: a Bot Deathmatch bot).
    pub merc: Option<MercMind>,
}

impl CaBot {
    pub fn new(profile: usize, template: Template, seed: u64, kit: roster::Kit, data: &BotData) -> Self {
        Self {
            profile,
            template,
            dice: Dice::new(seed),
            kit,
            armed: false,
            reach_fixed: false,
            ranged_reach: 0.0,
            thrown_reach: 0.0,
            state: BotState::Patrol,
            target: None,
            visible: false,
            head_only: false,
            tracking: Tracking::default(),
            cycle: HitCycle::default(),
            aim_offset: None,
            exact: false,
            passes: 0,
            sight_cooldown: 0.0,
            footsteps: math::Sense::default(),
            last_step_at: None,
            next_shot: 0.0,
            last_step: 0,
            last_gunfire: 0,
            last_call: 0,
            pending: None,
            attack_off: 0.0,
            fire_at: 0.0,
            chase_timer: None,
            held: None,
            burst_left: 0,
            pause_until: 0.0,
            last_clip: None,
            grenades: data.template.grenade_shots.at(template) as u32,
            next_move: 0.0,
            move_to: None,
            move_until: 0.0,
            dodge_next: 0.0,
            dodge_hit: false,
            crouch_until: 0.0,
            release: false,
            danger: None,
            throw: None,
            alarm: 0.0,
            last_relaxed: None,
            chat: Vec::new(),
            needs_start: true,
            patrol_wait: 0.0,
            merc_aim: false,
            merc_height: brain::MERC_HEIGHT,
            route: Vec::new(),
            route_goal: None,
            patrol_from: None,
            patrol_to: None,
            patrol_came: None,
            stuck_check: (0.0, Vec3::ZERO),
            jump: false,
            merc: None,
        }
    }
}

/// The bots, their console and their systems (the server's).
pub struct CaBotsPlugin;

impl Plugin for CaBotsPlugin {
    fn build(&self, app: &mut App) {
        use crate::map::MapScopedApp;
        let server = crate::core::authoritative;
        app.add_message::<chat::CaBotChat>()
            .add_message::<chat::CaBotEvent>()
            .add_message::<crate::core::Died>()
            .init_resource::<BotSettings>()
            .init_resource::<BotData>()
            .init_resource::<roster::Roster>()
            .init_resource::<brain::Stimuli>()
            .init_resource::<brain::PatrolClock>()
            .init_resource::<brain::BotFrames>()
            .map_scoped::<brain::NavJob>()
            .init_resource::<crate::map::MapHooks>()
            .add_systems(
                FixedUpdate,
                (
                    (roster::keep_quota, brain::ensure_nav, roster::arm, brain::think)
                        .chain()
                        .before(SimSet::Rules)
                        .before(crate::weapon::SelectWeapons),
                    (brain::post_stimuli, brain::camping, chat::chat)
                        .chain()
                        .after(SimSet::Weapons)
                        .after(crate::core::apply_damage),
                )
                    .run_if(server)
                    // While Combat Arms' bots are the gameplay's (its
                    // rules', or a minigame's that names them).
                    .run_if(crate::gameplay::bots_are(super::GAME)),
            );
        app.world_mut().resource_mut::<crate::map::MapHooks>().settle.push(brain::settle_nav);
        // The network's `kick <name>` and its quota at serving's start.
        crate::gameplay::GameplayAppExt::game_hook(app, super::GAME, crate::net::BotHooks {
            kick: |w, e| {
                kick(w, e);
                let mut s = w.resource_mut::<BotSettings>();
                s.quota = s.quota.saturating_sub(1);
                s.quota_set = true;
            },
            sync_quota: |w| {
                let n = w
                    .query_filtered::<(), (With<CaBot>, Without<crate::core::Npc>)>()
                    .iter(w)
                    .count() as u32;
                let mut s = w.resource_mut::<BotSettings>();
                if n > 0 {
                    s.quota = n;
                    s.quota_set = true;
                }
            },
        });
        resource_cvar::<BotSettings, u32>(app, "combat_arms:MaxPlayer", "Bots the server keeps in the game (Bot Deathmatch's player count).", |s| {
            &mut s.quota
        });
        resource_cvar::<BotSettings, u8>(
            app,
            "combat_arms:BotLevel",
            "Level of bots added from now on: 0 Easy, 1 Normal, 2 Veteran (3 training).",
            |s| &mut s.level,
        );
        resource_cvar::<BotSettings, u8>(app, "combat_arms:ca_bot_stop", "1: bots stand still (they still see, aim and fire).", |s| &mut s.stop);
        resource_cvar::<BotSettings, u8>(app, "combat_arms:ca_bot_dont_shoot", "1: bots never fire.", |s| &mut s.dont_shoot);
        app.console_command(
            "combat_arms:ca_bot_add",
            "ca_bot_add [alpha|bravo]: add a bot at BotLevel (default team: opposite the human); raises MaxPlayer.",
            |w, a| {
                let team = match a.first().map(|s| s.to_ascii_lowercase()) {
                    None => None,
                    Some(t) if t == "alpha" || t == "1" || t == "jsf" => Some(roster::ALPHA),
                    Some(t) if t == "bravo" || t == "2" || t == "creed" => Some(roster::BRAVO),
                    Some(t) => return Err(format!("bad team {t}")),
                };
                let e = add_bot(w, team).ok_or("no bot added")?;
                {
                    let mut s = w.resource_mut::<BotSettings>();
                    s.quota += 1;
                    s.quota_set = true;
                }
                let name = w.get::<Name>(e).map_or_else(|| e.to_string(), |n| n.as_str().to_string());
                Ok(Some(format!("added {name}")))
            },
        )
        .console_command(
            "combat_arms:ca_bot_kick",
            "ca_bot_kick [all|<name>]: remove every bot (MaxPlayer 0) or the one by that name.",
            |w, a| {
                let wanted = a.join(" ");
                let all = wanted.is_empty() || wanted.eq_ignore_ascii_case("all");
                let bots: Vec<Entity> = w
                    .query_filtered::<(Entity, Option<&Name>), (With<CaBot>, Without<crate::core::Npc>)>()
                    .iter(w)
                    .filter(|(_, n)| all || n.is_some_and(|n| n.as_str().eq_ignore_ascii_case(&wanted)))
                    .map(|(e, _)| e)
                    .collect();
                if !all && bots.is_empty() {
                    return Err(format!("no bot \"{wanted}\""));
                }
                let n = bots.len();
                for b in bots {
                    kick(w, b);
                }
                let mut s = w.resource_mut::<BotSettings>();
                s.quota = if all { 0 } else { s.quota.saturating_sub(n as u32) };
                s.quota_set = true;
                Ok(Some(format!("kicked {n} bots")))
            },
        )
        .console_command("combat_arms:ca_bots", "The bots: name, team, template, state, target, health.", |w, _| {
            let mut lines = Vec::new();
            let mut q = w.query::<(&CaBot, Option<&Name>, &crate::core::Team, &crate::core::Health)>();
            for (b, n, t, h) in q.iter(w) {
                lines.push(format!(
                    "{} team {} {} {:?} target {:?} hp {:.0}",
                    n.map_or("?", |n| n.as_str()),
                    t.0,
                    b.template.short(),
                    b.state,
                    b.target,
                    h.current * 100.0
                ));
            }
            Ok(Some(if lines.is_empty() { "no bots".into() } else { lines.join("\n") }))
        });
    }
}
