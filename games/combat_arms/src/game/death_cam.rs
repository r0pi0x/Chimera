//! Death, the death camera and the kill-cam screen
//! (specs/combat_arms/death_killcam.md): there is no replay; after a
//! death the client runs its own camera phases and respawn countdown, and
//! the server sends only facts.
//!
//! - `math`: the pull-back (3.5 s in every mode, 20 u above the body, 5 u
//!   a frame with the 0.9 pull-in, away from the damage's direction,
//!   world-clipped) and the watch orbit around a living player (the dead
//!   player's own mouse, zoom 40 u a notch), in the game's frame.
//!   `machine`: the phases, the targets (the killer if the enemy view is on
//!   and it lives, else living teammates by client id, then enemies with
//!   the option), the 8 s move-off, the keys (Space, Ctrl, Shift cycle; F7
//!   fast respawn), Hunted's hold and free cameras, and the respawn
//!   countdown (gauge, timer, the request). `screen`: what the death
//!   screen shows (the last damage figure, hit zone, tips from the guide
//!   data).
//! - Here the engine's side. The server's facts: `CaSlot` (each player's
//!   client id), `CaKillMe` (the times each player killed a victim, the
//!   kill message's count), `CaKillCamFlag` (the room's Kill Cam option, on
//!   the match), `CaFastRespawn`, `CaRelentless`, `CaHeld` (a held
//!   joiner), and `spawns::request_respawn_with` (the request's check). The
//!   client's `CaDeathCam` runs the machine each rendered frame on what it
//!   can see: the own player's state, the kill message (`core::Died`), the
//!   damage report (`core::DamageTaken` on a network client, else
//!   `core::Damage`), the players and the world. Its outputs are the camera
//!   pose in the engine's frame, the watched player, the death screen's
//!   values and the respawn request, sent through the rules' own path:
//!   `spawns::request_respawn_with`, or on a network client
//!   `net::CaRespawnRequest`.
//! - Cvars: `Enemydeadcam` (the enemy view, written at a match's start
//!   from the room's option `ca_killcam`: 0 Off, 1 On, -1 the mode's
//!   default, Off but Hunted On).
//!
//! The front end (`ui::hud::death`) draws the screen on the install's
//! `Hud_KillCam` layout by role and puts the camera on the shell's.
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear):
//! - Clocks: every death clock (the pull-back, the 8 s, the wait) is the
//!   game's time (`SimClock`, a network client's fixed clock), not the OS
//!   millisecond tick the spec has; the camera moves once per rendered
//!   frame.
//! - The world rays hit anything on the gameplay layers but characters
//!   (props too: the world model/prop split isn't known); the sound
//!   listener isn't moved to the camera; Hunted's hold rides the body's
//!   place with the saved view (no `Eyes` socket in `CaPose`).
//! - The watch phase's first target is resolved in the frame it starts
//!   (the spec checks "one frame later"): the killer is never shown for a
//!   frame when the enemy view is off. After the respawn request's blip
//!   (Quirk 12) the watch goes to the next living player after the one
//!   watched.
//! - The damage direction and amount come from the last `DamageTaken` (or
//!   `Damage`), the hit location from the killing hit (`Hurt` carries no
//!   hit location); groin hits aren't told apart from the stomach's. The
//!   figure doubles the amount for blasts and burns before truncating (the
//!   spec says "truncated, then doubled", its case 22 has 37.8 give 75).
//! - Not specified, so not there: the killing weapon's large icon (the
//!   kill feed's icon stands in), a killer's rank icon, and the numbers of
//!   the guide's damage-type, item and weapon conditions (a tip asking for
//!   one isn't chosen), the user type and the map (those conditions
//!   pass). The guide file is the attributes archive's `guide.txt`.
//! - Relentless (`CaRelentless`) and the fast-respawn counts and premium
//!   pass (`CaFastRespawn`) are granted by `perks::wire_respawn` from the
//!   player's loadout (the console's `ca_perks`, `ca_fast_respawns`; no
//!   lobby); the fast respawn item isn't there.
//! - Clients get a player's `CaKillMe`, `CaSlot` and `CaFastRespawn` as
//!   every replicated component (not only the owner's).
//! - Free-camera speed: velocity from the acceleration over the frame's
//!   length, damped by 0.9, moves the camera by that over the frame
//!   (Open question 11).
//! - The tick sound is a list of whole seconds (`Machine::ticks`) nothing
//!   plays; no sound file is wired.
//! - Cinematics and the AI freeze don't hide the screen (only the map
//!   camera, `core::MapView`, does).

pub mod machine;
pub mod math;
pub mod screen;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use self::{
    machine::{Cand, Frame, Id, Input, Killer, Machine, Phase},
    math::Clip,
    screen::{DamageView, DeathScreen, FastView, KillerView, RespawnView, TargetView, TipView, WeaponView},
};
use super::{
    inventory::UNIT,
    rules::{CaMatch, CaMatchView, CaRuleSettings, LAST_MAN_STANDING, current_mode, mercs::CaMerc, spawns::CaHeld},
};
use crate::{
    console::resource_cvar,
    core::{
        Damage, DamageTaken, Dead, Died, Health, Intent, LocalPlayer, MapLoads, NetRole, Npc, SOLID_LAYERS, SimClock,
        Spectating, Team,
    },
    net::{NetAppExt, ordered::OrderedAppExt},
    weapon::Inventory,
};

/// The guide data (the in-game tips, `InGame<i>` records) in the
/// install's attribute archive (a guess at the name: `heavy::death_screen`
/// checks it).
pub const GUIDE_FILE: &str = "attributes/guide.txt";

/// The install's tips (none without it, or in the test harness).
pub fn install_tips(data: &super::data::CaData) -> Vec<screen::Tip> {
    if data.builtin_only() {
        return Vec::new();
    }
    let Some(mount) = super::INFO.mount() else { return Vec::new() };
    let Ok(bytes) = mount.read(GUIDE_FILE) else { return Vec::new() };
    match lithtech_formats::attributes::Document::parse(&bytes) {
        Ok(doc) => screen::parse_guide(&doc),
        Err(e) => {
            warn!("combat_arms death screen: {GUIDE_FILE}: {e}; no tips");
            Vec::new()
        }
    }
}

/// A player's client id: the order the dead cycle through (5). The
/// server gives each player the next free one.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaSlot(pub u16);

/// The next client id to give out.
#[derive(Resource, Default, Debug)]
pub struct CaSlots(pub u16);

/// How many times each other player killed this one in this match
/// (team kills included, suicides and warm-up kills not), by the
/// killer's client id; sent with the kill message (8).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaKillMe(pub Vec<(u16, u16)>);

impl CaKillMe {
    pub fn count(&self, killer: u16) -> u32 {
        self.0.iter().find(|(k, _)| *k == killer).map_or(0, |(_, n)| *n as u32)
    }
}

/// The room's Kill Cam option as the match tells the clients (the
/// enemy view).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaKillCamFlag(pub bool);

/// A player's fast respawns (the lobby's counts; 6, 9).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaFastRespawn {
    pub left: u32,
    pub max: u32,
    /// The premium pass: unlimited, none spent.
    pub premium: bool,
}

/// The Relentless perk: the client's respawn wait becomes
/// min(wait, `RespawnTime`) s (gear_perks.md).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CaRelentless(pub f32);

/// The console variable `Enemydeadcam` (1: the dead may follow the
/// killer and, with no teammate alive, enemies; the death screen shows
/// the killer's health and armour).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaEnemyCam(pub u8);

/// The room's Kill Cam option: 1 on, 0 off, -1 the mode's default (off;
/// Hunted on).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaKillCamRoom(pub i32);

impl Default for CaKillCamRoom {
    fn default() -> Self {
        Self(-1)
    }
}

impl CaKillCamRoom {
    /// The enemy view the room starts with in mode `key`.
    pub fn enemy_view(&self, key: &str) -> bool {
        match self.0 {
            0 => false,
            -1 => key == "HG",
            _ => true,
        }
    }
}

/// What the player did this frame (the front end writes it; tests too).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CaDeathInput(pub Input);

/// The kill message's facts, kept for the screen (8): they don't change
/// afterwards.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KillFact {
    pub killer: Killer,
    /// The killer's name, health and armour when the message arrived.
    pub name: Option<String>,
    pub hp: f32,
    pub ap: f32,
    pub teammate: bool,
    pub weapon: Option<String>,
    pub hitgroup: crate::core::Hitgroup,
    pub kind: crate::core::DamageKind,
}

/// The last damage report (C), in the game's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LastHit {
    pub dir: Vec3,
    /// Hit points (the damage message's health fraction x 100).
    pub amount: f32,
    pub kind: crate::core::DamageKind,
}

/// The rules the machine is given this frame (worked out where the world
/// is at hand: `eligibility`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rules {
    pub counts: bool,
    pub fast_ok: bool,
    pub fast_mode: bool,
    pub hunted: bool,
    pub any_team: bool,
    pub wait_ms: f64,
    pub perk: bool,
}

/// The death camera of this machine's player.
#[derive(Resource, Default)]
pub struct CaDeathCam {
    pub machine: Machine,
    pub rules: Rules,
    pub kill: Option<KillFact>,
    pub hit: Option<LastHit>,
    /// The own state last frame.
    pub was_dead: bool,
    /// The camera's pose in the engine's frame, when the death camera
    /// (or the waiting camera) holds it.
    pub pose: Option<(Vec3, Quat)>,
    /// The player the camera is on (the HUD's tag, the target panel).
    pub watched: Option<Entity>,
    /// The death screen's values, and whether it is up.
    pub screen: DeathScreen,
    pub screen_up: bool,
    /// "Waiting": a held joiner's or observer's free camera.
    pub waiting: bool,
    /// The guide data's tips and the room they are chosen for.
    pub tips: Vec<screen::Tip>,
    pub room: screen::RoomFacts,
    tip: Option<TipView>,
    map_loads: u32,
}

impl CaDeathCam {
    /// The death camera is on the view (dead or waiting).
    pub fn active(&self) -> bool {
        self.machine.is_dead() || self.waiting
    }

    /// The death-camera phase.
    pub fn phase(&self) -> Phase {
        self.machine.phase
    }
}

/// The death camera systems, after the HUD's state and before the shell
/// places the camera.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeathCamSet;

pub struct CaDeathCamPlugin;

impl Plugin for CaDeathCamPlugin {
    fn build(&self, app: &mut App) {
        let tips = install_tips(&super::data::CaData::of(app));
        app.init_resource::<CaSlots>()
            .init_resource::<CaEnemyCam>()
            .init_resource::<CaKillCamRoom>()
            .init_resource::<CaDeathInput>()
            .insert_resource(CaDeathCam {
                tips,
                ..default()
            })
            .add_message::<Died>()
            .add_message::<crate::game::net::CaRespawnRequest>()
            .add_message::<Damage>()
            .add_message::<DamageTaken>();
        resource_cvar::<CaEnemyCam, u8>(
            app,
            "combat_arms:Enemydeadcam",
            "1: the dead may follow their killer (and enemies when no teammate lives); the death screen shows the killer's health and armour.",
            |c| &mut c.0,
        );
        resource_cvar::<CaKillCamRoom, i32>(
            app,
            "combat_arms:ca_killcam",
            "The room's Kill Cam option (KillCam_On / KillCam_Off): 1 on, 0 off, -1 the mode's default (off; Hunted on).",
            |c| &mut c.0,
        );
        app.add_systems(
            Update,
            (
                (assign_slots, count_kills, write_flag).run_if(not_client),
                sync_enemy_cam,
                eligibility,
                drive,
            )
                .chain()
                .in_set(DeathCamSet),
        );
        app.net_part("combat_arms_death_cam", register);
    }
}

fn not_client(role: Option<Res<NetRole>>) -> bool {
    role.is_none_or(|r| *r != NetRole::Client)
}

/// The protocol: the components above, in this order on every side.
fn register(app: &mut App) {
    app.replicate::<CaSlot>()
        .ordered::<CaSlot>()
        .replicate::<CaKillMe>()
        .ordered::<CaKillMe>()
        .replicate::<CaKillCamFlag>()
        .ordered::<CaKillCamFlag>()
        .replicate::<CaFastRespawn>()
        .ordered::<CaFastRespawn>()
        .replicate::<CaRelentless>()
        .ordered::<CaRelentless>()
        .replicate::<CaHeld>()
        .ordered::<CaHeld>();
}

// ---------------------------------------------------------------------------
// The server's facts

/// Every player gets the next client id (the order the dead cycle in).
fn assign_slots(
    new: Query<Entity, (With<Intent>, Without<CaSlot>, Without<Npc>)>,
    mut slots: ResMut<CaSlots>,
    mut commands: Commands,
) {
    let mut new: Vec<Entity> = new.iter().collect();
    new.sort();
    for e in new {
        slots.0 += 1;
        commands.entity(e).insert(CaSlot(slots.0));
    }
}

/// The kill-me counts (8): a death by another player, a team kill
/// included, not in a warm-up, adds one to the victim's count of the
/// killer. A new map starts them over.
fn count_kills(
    mut died: MessageReader<Died>,
    slots: Query<&CaSlot>,
    mut counts: Query<&mut CaKillMe>,
    matches: Option<Res<CaMatch>>,
    loads: Option<Res<MapLoads>>,
    mut seen: Local<u32>,
    mut commands: Commands,
) {
    let now = loads.map_or(0, |l| l.0);
    if *seen != now {
        *seen = now;
        for mut c in &mut counts {
            c.0.clear();
        }
    }
    for d in died.read() {
        let Some(k) = d.attacker.filter(|k| *k != d.entity) else { continue };
        if matches.as_ref().is_some_and(|m| !m.counting()) {
            continue;
        }
        let Ok(slot) = slots.get(k) else { continue };
        match counts.get_mut(d.entity) {
            Ok(mut c) => match c.0.iter_mut().find(|(s, _)| *s == slot.0) {
                Some((_, n)) => *n += 1,
                None => c.0.push((slot.0, 1)),
            },
            Err(_) => {
                commands.entity(d.entity).insert(CaKillMe(vec![(slot.0, 1)]));
            }
        }
    }
}

/// The room's Kill Cam option on the match entity, for the clients.
fn write_flag(
    matches: Query<(Entity, Option<&CaKillCamFlag>), With<crate::game::net::NetCaMatch>>,
    settings: Res<CaRuleSettings>,
    room: Res<CaKillCamRoom>,
    mut commands: Commands,
) {
    let want = CaKillCamFlag(room.enemy_view(&settings.mode));
    for (e, have) in &matches {
        if have != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// `Enemydeadcam` from the room's option at a match's start (a new map,
/// or the mode or option changing): the room's own on a server or alone,
/// the match's flag on a client.
fn sync_enemy_cam(
    role: Option<Res<NetRole>>,
    settings: Res<CaRuleSettings>,
    room: Res<CaKillCamRoom>,
    flags: Query<&CaKillCamFlag>,
    loads: Option<Res<MapLoads>>,
    mut enemy: ResMut<CaEnemyCam>,
    mut last: Local<Option<(bool, u32)>>,
) {
    let client = role.is_some_and(|r| *r == NetRole::Client);
    let want = if client {
        match flags.iter().next() {
            Some(f) => f.0,
            None => return,
        }
    } else {
        room.enemy_view(&settings.mode)
    };
    let key = (want, loads.map_or(0, |l| l.0));
    if *last != Some(key) {
        *last = Some(key);
        enemy.0 = want as u8;
    }
}

// ---------------------------------------------------------------------------
// The client

/// The modes with a fast respawn (6): TDM, One Man Army, CTF, Spy Hunt,
/// Bombing Run, Seize and Secure, Hired Guns, Arms Race and VIP
/// Elimination.
pub const FAST_MODES: &[&str] = &["TDM", "FFA", "CTF", "BNT", "SBT", "HQM", "SQM", "VWRM", "VTDM"];

/// The rules the machine is given: whether the mode counts a respawn
/// countdown for this player, its wait (7 s, Relentless's shorter) and
/// whether F7 may ask (the mode, the counts or the premium pass).
fn eligibility(world: &mut World) {
    let me = chimera_core::query_cache::first::<With<LocalPlayer>>(world);
    let mode = current_mode(world).or_else(|| world.get_resource::<CaMatchView>().and_then(|v| v.mode()));
    let mut rules = Rules::default();
    let mut room = screen::RoomFacts::default();
    if let Some(mode) = mode {
        rules.hunted = mode.key == "HG";
        rules.any_team = rules.hunted || mode.id == LAST_MAN_STANDING;
        rules.counts = mode.respawns && me.is_none_or(|e| (mode.hooks.may_respawn)(world, e));
        rules.fast_mode = FAST_MODES.contains(&mode.key);
        room = screen::RoomFacts {
            mode_type: screen::game_type(mode.key),
        };
        let settings_wait = world.get_resource::<CaRuleSettings>().map_or(7.0, |s| s.respawn_wait_client as f64);
        rules.wait_ms = settings_wait * 1000.0;
        if let Some(r) = me.and_then(|e| world.get::<CaRelentless>(e)).copied() {
            rules.wait_ms = rules.wait_ms.min(r.0 as f64 * 1000.0);
            rules.perk = true;
        }
        if let Some(e) = me {
            let f = world.get::<CaFastRespawn>(e).copied().unwrap_or_default();
            rules.fast_ok = rules.fast_mode && (f.premium || f.left > 0);
        }
    }
    let mut cam = world.resource_mut::<CaDeathCam>();
    cam.rules = rules;
    cam.room = room;
}

/// The world as the camera's rays meet it: gameplay geometry, characters
/// left out.
struct WorldClip<'a, 'w, 's> {
    spatial: &'a SpatialQuery<'w, 's>,
    characters: Vec<Entity>,
}

impl Clip for WorldClip<'_, '_, '_> {
    fn hit(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        let (a, b) = (math::to_world(from, UNIT), math::to_world(to, UNIT));
        let (dir, length) = Dir3::new_and_length(b - a).ok()?;
        let filter = SpatialQueryFilter::from_excluded_entities(self.characters.iter().copied()).with_mask(SOLID_LAYERS);
        let hit = self.spatial.cast_ray(a, dir, length, true, &filter)?;
        Some(math::from_world(a + *dir * hit.distance, UNIT))
    }
}

/// The game clock this machine's death clocks run on.
fn game_now(role: Option<&NetRole>, sim: Option<&SimClock>, fixed: &Time<Fixed>, time: &Time) -> f64 {
    if role == Some(&NetRole::Client) {
        fixed.elapsed_secs_f64()
    } else {
        sim.map_or_else(|| time.elapsed_secs_f64(), |c| c.now)
    }
}

/// One rendered frame of the death camera: the facts in, the machine run,
/// the pose, the watched player and the screen out.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    (time, fixed, sim, role): (Res<Time>, Res<Time<Fixed>>, Option<Res<SimClock>>, Option<Res<NetRole>>),
    mut cam: ResMut<CaDeathCam>,
    input: Res<CaDeathInput>,
    enemy: Res<CaEnemyCam>,
    loads: Option<Res<MapLoads>>,
    (mut died, mut damage, mut taken): (MessageReader<Died>, MessageReader<Damage>, MessageReader<DamageTaken>),
    local: Option<Single<(Entity, &Transform, &Intent, Option<&Dead>, Has<Spectating>, Has<CaHeld>, Option<&CaSlot>, Option<&Team>, Option<&CaKillMe>), With<LocalPlayer>>>,
    players: Query<(Entity, &Transform, Option<&CaSlot>, Option<&Team>, &Health, Has<Dead>, Has<CaHeld>, Has<Spectating>, Option<&Name>), (With<Intent>, Without<Npc>)>,
    (mercs, armour, inventory, weapons): (
        Query<(Entity, &CaMerc, &Transform, &Health, Has<Dead>)>,
        Query<&crate::weapon::Armor>,
        Query<&Inventory>,
        Query<&crate::weapon::Weapon>,
    ),
    names: Query<Option<&Name>>,
    (spatial, characters): (SpatialQuery, Query<Entity, With<Intent>>),
    (fast, map_view): (Query<&CaFastRespawn>, Query<(), (With<LocalPlayer>, With<crate::core::MapView>)>),
    mut request: MessageWriter<crate::game::net::CaRespawnRequest>,
    mut commands: Commands,
) {
    let cam = &mut *cam;
    let now = game_now(role.as_deref(), sim.as_deref(), &fixed, &time);
    let client = role.as_deref() == Some(&NetRole::Client);
    // A new map: the free cameras' boost and the static camera.
    let now_loads = loads.map_or(0, |l| l.0);
    if cam.map_loads != now_loads {
        cam.map_loads = now_loads;
        cam.machine.map_loaded(cam.rules.hunted);
    }
    // The damage report (C): the server's on a client, else the damage
    // dealt here.
    let me = local.as_ref().map(|l| l.0);
    let hits: Vec<Damage> = if client {
        damage.clear();
        taken.read().map(|t| t.damage.clone()).collect()
    } else {
        taken.clear();
        damage.read().cloned().collect()
    };
    for d in hits.iter().filter(|d| Some(d.target) == me && d.amount > 0.0) {
        cam.hit = Some(LastHit {
            dir: math::mirror(d.dir),
            amount: d.amount * 100.0,
            kind: d.kind,
        });
    }
    // The kill message (11): who, with what, and the killer's health and
    // armour as the client knows them now (a snapshot, Quirk 11).
    let slot_key = |e: Entity| -> Id {
        players
            .get(e)
            .ok()
            .map_or(OBJECT_BASE + e.index_u32(), |p| p.2.map_or(PLAYERLESS_BASE + e.index_u32(), |s| s.0 as Id))
    };
    for d in died.read() {
        if Some(d.entity) != me {
            continue;
        }
        let attacker = d.attacker;
        // A mercenary's kill is its leader's, the camera follows the
        // mercenary under the leader's name.
        let (killer_entity, object) = match attacker.and_then(|a| mercs.get(a).ok().map(|(_, m, ..)| (a, m.leader))) {
            Some((merc, leader)) => (Some(leader), Some(merc)),
            None => (attacker, None),
        };
        let killer = match killer_entity {
            Some(k) if Some(k) == me => Killer::Suicide,
            Some(k) => Killer::Player {
                id: slot_key(k),
                object: object.map(|m| OBJECT_BASE + m.index_u32()),
            },
            None => Killer::World,
        };
        let weapon = d.damage.weapon.map(str::to_string).or_else(|| {
            let k = attacker.filter(|k| Some(*k) != me)?;
            let w = inventory.get(k).ok()?.active?;
            Some(weapons.get(w).ok()?.id.to_string())
        });
        let k = killer_entity.filter(|k| Some(*k) != me);
        let team = |e: Entity| players.get(e).ok().and_then(|p| p.3).map(|t| t.0);
        cam.kill = Some(KillFact {
            killer,
            name: k.and_then(|k| names.get(k).ok().flatten()).map(|n| n.to_string()),
            hp: k.and_then(|k| players.get(k).ok()).map_or(0.0, |p| p.4.current * 100.0 + 1e-3),
            ap: k.and_then(|k| armour.get(k).ok()).map_or(0.0, super::weapons::points),
            teammate: k.is_some_and(|k| team(k).is_some() && team(k) == local.as_ref().and_then(|l| l.7).map(|t| t.0)),
            weapon,
            hitgroup: d.damage.hitgroup,
            kind: d.damage.kind,
        });
        if cam.hit.is_none() {
            cam.hit = Some(LastHit {
                dir: math::mirror(d.damage.dir),
                amount: 0.0,
                kind: d.damage.kind,
            });
        }
    }
    let Some(local) = local else {
        cam.pose = None;
        cam.watched = None;
        cam.screen_up = false;
        cam.waiting = false;
        return;
    };
    let (me, at, intent, dead, spectating, held, slot, team, kill_me) = *local;
    let slot_id = slot.map_or(PLAYERLESS_BASE + me.index_u32(), |s| s.0 as Id);
    let my_team = team.map_or(0, |t| t.0);
    let waiting = held || spectating;
    let is_dead = dead.is_some() && !waiting;
    let body = math::from_world(at.translation, UNIT);
    // The view when the death came, in the machine's angles (ψ turns
    // right, θ looks down).
    let view = (-intent.yaw, (-intent.pitch).clamp(-math::PITCH_LIMIT, math::PITCH_LIMIT));
    // The players, in client id order, and the mercenaries the enemy view
    // may follow.
    let mut list: Vec<(Cand, Entity)> = players
        .iter()
        .map(|(e, t, s, team, h, dead, held, spec, _)| {
            (
                Cand {
                    id: s.map_or(PLAYERLESS_BASE + e.index_u32(), |s| s.0 as Id),
                    team: team.map_or(0, |t| t.0),
                    alive: !dead && h.current > 0.0 && !held && !spec,
                    at: math::from_world(t.translation, UNIT),
                },
                e,
            )
        })
        .collect();
    list.sort_by_key(|(c, e)| (c.id, *e));
    let cands: Vec<Cand> = list.iter().map(|(c, _)| *c).collect();
    let objects: Vec<Cand> = mercs
        .iter()
        .map(|(e, m, t, h, d)| Cand {
            id: OBJECT_BASE + e.index_u32(),
            team: players.get(m.leader).ok().and_then(|p| p.3).map_or(0, |t| t.0),
            alive: !d && h.current > 0.0,
            at: math::from_world(t.translation, UNIT),
        })
        .collect();
    // The transition into the dead state.
    if is_dead && !cam.was_dead {
        let killer = cam.kill.as_ref().map_or(Killer::Unknown, |k| k.killer);
        let since = dead.map_or(now, |d| d.since).min(now);
        cam.machine.died(since, killer, view);
        // The countdown starts on the dead state's first frame.
    }
    if !is_dead && cam.was_dead {
        cam.machine.alive();
        cam.kill = None;
        cam.hit = None;
        cam.tip = None;
        cam.waiting = false;
    }
    cam.was_dead = is_dead;
    // The kill message may come a frame after the state: the killer then.
    if is_dead && cam.machine.killer == Killer::Unknown && let Some(k) = &cam.kill {
        cam.machine.killer = k.killer;
    }
    let clip = WorldClip {
        spatial: &spatial,
        characters: characters.iter().collect(),
    };
    let rules = cam.rules;
    let f = Frame {
        now,
        dead: is_dead,
        waiting,
        me: slot_id,
        my_team,
        body,
        damage: cam.hit.map_or(Vec3::ZERO, |h| h.dir),
        players: &cands,
        objects: &objects,
        enemy_view: enemy.0 != 0,
        any_team: rules.any_team,
        hunted: rules.hunted,
        counts: rules.counts,
        wait_ms: rules.wait_ms,
        fast_ok: rules.fast_ok,
        view,
        input: Input {
            dt: input.0.dt,
            ..input.0
        },
        clip: &clip,
    };
    if waiting {
        cam.machine.wait_frame(&f);
        cam.waiting = true;
    } else if is_dead {
        cam.machine.frame(&f);
        cam.waiting = false;
    } else {
        cam.waiting = false;
    }
    // The respawn request goes out the frame it is due, through the rules'
    // own path (S5).
    if let Some(fast_flag) = cam.machine.request.take() {
        if client {
            request.write(crate::game::net::CaRespawnRequest { fast: fast_flag });
        } else {
            commands.queue(move |w: &mut World| {
                let _ = super::rules::spawns::request_respawn_with(w, me, fast_flag);
            });
        }
    }
    // Outputs.
    cam.pose = cam
        .machine
        .pose
        .filter(|_| is_dead || waiting)
        .map(|p| (math::to_world(p.at, UNIT), math::to_rotation(p.heading, p.pitch)));
    let by_id = |id: Id| list.iter().find(|(c, _)| c.id == id).map(|(_, e)| *e);
    cam.watched = if is_dead && cam.machine.phase == Phase::Watch {
        cam.machine.target.and_then(by_id)
    } else {
        None
    };
    // The death screen (8).
    cam.screen_up = is_dead && map_view.is_empty();
    cam.screen = if is_dead {
        let target = cam.watched.map(|e| {
            let slot = players.get(e).ok().and_then(|p| p.2).map(|s| s.0);
            let n = kill_me.zip(slot).map_or(0, |(k, s)| k.count(s));
            TargetView {
                name: names.get(e).ok().flatten().map_or_else(|| "Player".into(), |n| n.to_string()),
                kills_you: (n > 0).then_some(n),
                free_cam_info: rules.hunted && cam.machine.phase != Phase::None,
            }
        });
        let kill = cam.kill.clone();
        let killer = kill.as_ref().filter(|k| matches!(k.killer, Killer::Player { .. })).and_then(|k| {
            Some(KillerView {
                name: k.name.clone()?,
                cam_on: enemy.0 != 0 || k.teammate,
                hp: screen::three_digits(k.hp),
                ap: screen::three_digits(k.ap),
            })
        });
        let weapon = kill.as_ref().filter(|k| k.killer != Killer::Suicide).and_then(|k| {
            k.weapon.clone().map(|id| WeaponView {
                name: id.rsplit(':').next().unwrap_or(&id).to_string(),
                id,
            })
        });
        let damage = cam.hit.zip(kill.as_ref()).map(|(h, k)| DamageView {
            text: screen::damage_figure(h.amount, screen::doubled(h.kind)).to_string(),
            zone: screen::zone_of(k.hitgroup, false),
        });
        let respawn = cam.machine.countdown.map(|c| RespawnView {
            gauge: c.gauge(now),
            timer: cam.machine.text.clone().unwrap_or_else(|| c.text(now)),
            perk: rules.perk,
            fast: rules.fast_mode.then(|| {
                let f = fast.get(me).copied().unwrap_or_default();
                if f.premium { FastView::Premium } else { FastView::Free { left: f.left, max: f.max } }
            }),
        });
        let banner = respawn.is_some().then(|| cam.tip.clone()).flatten().or_else(|| {
            if respawn.is_none() {
                return None;
            }
            let death = screen::DeathFacts {
                suicide: kill.as_ref().is_some_and(|k| k.killer == Killer::Suicide),
                ..Default::default()
            };
            let seed = (me.to_bits() ^ cam.machine.t0().unwrap_or(0.0).to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            screen::pick_tip(&cam.tips, &cam.room, &death, seed)
        });
        cam.tip = banner.clone();
        DeathScreen {
            killer,
            weapon,
            damage,
            target,
            banner,
            respawn,
        }
    } else {
        DeathScreen::default()
    };
}

/// An object's key (a mercenary the enemy view follows) is this plus its
/// entity index; a player without a `CaSlot` is placed after the slotted
/// ones.
const OBJECT_BASE: Id = 0x2_0000;
const PLAYERLESS_BASE: Id = 0x1_0000;
