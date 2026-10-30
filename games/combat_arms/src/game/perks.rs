//! Perks (specs/combat_arms/gear_perks.md sections 13 to 16; numbers
//! below are its): the abilities a player's loadout (`gear::CaLoadout`)
//! switches on, their state machine and what each changes.
//!
//! - `CaPerks` (every side, derived each tick from the loadout, the mode
//!   and the body): the abilities working now, with their level (0 basic,
//!   1 Expert). The room option "Perks" is `CaPerksEnabled` (the console's
//!   `ca_perks_enabled`); an infected body, a VIP and the modes a perk is
//!   off in (Relentless is on in only twelve, Parting Gift is off in
//!   Quarantine, Spoils of War and Opportunist in Arms Race) have none of
//!   the affected ones.
//! - `CaPerkState` (the server's, per player): kills, the multi-kill
//!   counter, the waits, cooldowns, buffs and used-up marks. `perk_tick`
//!   runs it each tick before the movement (the spec's once per 20 Hz
//!   frame; ours is the tick): a wait (movement keys cancel it) ends in
//!   the perk beginning, an in-use perk runs its time, a cooldown starts
//!   on a successful start.
//! - `CaPerkEffects` (replicated, ordered): the flags the movement, the
//!   weapons and the effects read, written each tick from the state: the
//!   stamina gauge's extra drain, free sprint (War Cry, Virulent), no
//!   recovery (Awareness, Steady Hand, Inner Focus), Victory Lap's sprint
//!   speed without the sprint flag, Steady Hand's still scope, Eagle
//!   Eye's spread cut, Specter's and Awareness's footstep scaling.
//! - Active perks start with `ca_perk <1-4>` (the use-perk key plus a
//!   number; a client's goes as `CaPerkRequest`): the position is the
//!   perk's among the working ones, in the order equipped. The server
//!   checks the perk's local conditions (Awareness needs stamina, Inner
//!   Focus needs missing health, Spoils of War a dropped weapon within
//!   `TargetDistance`) then starts it or, with a `WaitTime`, waits.
//! - Deaths (`perk_deaths`): Victory Lap's multi-kill counter (windows
//!   strictly under 5000 ms), Resourceful's roll and box, Parting Gift's grenade.
//! - Supplies (`supplies`): Resourceful's boxes and Warlord's supply
//!   point (`CaPerkBox`, `CaSupplyBox`; the server's, not shown to
//!   clients).
//! - The Pitcher bar (`CaPerkState::pitcher`): grows 1 per 8 ms while
//!   fire is held with a thrown weapon in hand and the player isn't
//!   sprinting; `throw_force` is what a throw takes.
//! - Fast respawn and Relentless reach the death camera and the respawn
//!   request as its `CaRelentless` (`respawn_wait`) and `CaFastRespawn`
//!   (the loadout's counts and pass), set by `wire_respawn`.
//!
//! Corrections 2026-10-22 (gear_perks.md), parity with the original,
//! faithful quirks marked: Resourceful's medikit heals and repairs to
//! full and is refused at full health and armour (the box stays); its ammo
//! box refills only the last SMG or assault rifle switched to
//! (`CaLastPrimary`; machine guns and snipers get nothing, quirk) and only
//! below the record's `MaxAmount`, so a set's raised cap never refills
//! (quirk); the full-ammo box also refills thrown weapons, mines and air
//! strikes; a box that refilled nothing stays. Warlord's point needs the
//! use key held within 100 u, the owner's team only, follows the gun type
//! (a switch to another weapon of the type keeps the charge), and goes
//! when the round is decided. Opportunist removes the body (`CaBody`s of
//! the character; the character is marked `CaLooted`) and the weapons
//! lie at the torso +-20 u, reserved for the looter for `OwnerTime`, then
//! for the looter's team only (the spec's wording; quirk); an empty body
//! fails with no cooldown. Parting Gift is the stock frag on a 2.5 s fuse,
//! whatever the dead carried (quirk). War Cry and Eagle Eye include dead
//! teammates in range (quirk; whether a respawn clears the buff is Open
//! question 11, we keep it to its end).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Timers are checked each tick, not once per 20 Hz server frame.
//! - Resourceful's boxes are used on touch (within 60 u) and aren't shown
//!   to clients (open question 6); mission items (the objective class) are
//!   skipped by the full-ammo box, the original's "ignore refills" flag is
//!   not read.
//! - Warlord's box appears 200 u ahead of the thrower at its floor level
//!   (no flight from the hull centre, no impact sound, can't be shot
//!   down: open question 10); the gun types without a charge time start
//!   none (`RechargeDefaultTime` isn't used: open question 7).
//! - Opportunist takes the nearest dead body's carried primaries other
//!   than the one it dropped, the torso is the hull centre, the body is
//!   not frozen while the perk waits; Spoils of War takes the nearest
//!   dropped weapon within the distance without a sight test or an owner
//!   check; Caution has no indicator to show; Parting Gift's grenade is
//!   the stock M67's data (the perk's `WeaponID` records are not read)
//!   with the perk's area damage.
//! - The weapon restrictions that switch Parting Gift off and "knife
//!   only" rooms for Warlord aren't read; Spy Hunt's super spy keeps
//!   Inner Focus.
//! - HUD positions count every working perk in the order equipped (the
//!   spec says only "the perk in that HUD position").

use chimera_core::query_cache::cached_query;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    death_cam::{CaFastRespawn, CaRelentless},
    gear::{CaGearData, CaLoadout, records::{GearData, PerkKind, PerkRecord}},
    grenades,
    inventory::{self, Dropped, UNIT},
    movement::{CaInfected, CaMovement},
    rules::{CaMatchView, Phase, escort::CaVip},
    weapons::zoom::CaZoom,
};
use crate::{
    core::{self, Damage, Dead, Died, Health, Intent, LocalPlayer, MovementState, NetRole, RoundRestarts, SimClock, SimSet, Team},
    console::ConsoleAppExt,
    net::{NetAppExt, ordered::OrderedAppExt, server::Player},
    weapon::{Inventory, Magazine, Weapon, grenade::Throwable, random::shared_random},
};

/// The perks by their `Type`.
pub const RESOURCEFUL: i32 = 1;
pub const AWARENESS: i32 = 2;
pub const SPECTER: i32 = 3;
pub const VICTORY_LAP: i32 = 4;
pub const STEADY_HAND: i32 = 5;
pub const PITCHER: i32 = 6;
pub const PARTING_GIFT: i32 = 7;
pub const SPOILS_OF_WAR: i32 = 8;
pub const OPPORTUNIST: i32 = 9;
pub const RELENTLESS: i32 = 10;
pub const WARLORD: i32 = 11;
pub const WAR_CRY: i32 = 12;
pub const EAGLE_EYE: i32 = 13;
pub const CAUTION: i32 = 17;
pub const INNER_FOCUS: i32 = 18;

/// The multi-kill window (13/Constants), s.
pub const MULTI_KILL_WINDOW: f64 = 5.0;
/// A box is picked up within this distance (u).
pub const PICKUP_RANGE: f32 = 60.0;
/// A supply point's touch distance (u) and how far ahead it lands (u).
pub const SUPPLY_RANGE: f32 = 100.0;
pub const SUPPLY_AHEAD: f32 = 200.0;
/// A body's backpack weapons spawn within this of its torso (u).
pub const LOOT_SPREAD: f32 = 20.0;

/// The modes Relentless works in (14): Elimination, VIP Elimination, clan
/// Elimination, One Man Army, Arms Race, CTF, clan CTF, Spy Hunt, Bombing
/// Run, Seize and Secure and Hired Guns (both).
pub const RELENTLESS_MODES: &[&str] = &["TDM", "VTDM", "CDM", "FFA", "VWRM", "CTF", "CCTF", "BNT", "SBT", "HQM", "SQM", "SQM2"];
/// The modes whose players are all enemies.
const EVERYONE_ENEMIES: &[&str] = &["FFA", "VWRM", "LMS", "HG"];

/// The room option "Perks" (`EnablePerkSkillSetting`, game_modes.md S2).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaPerksEnabled(pub bool);

impl Default for CaPerksEnabled {
    fn default() -> Self {
        Self(true)
    }
}

/// The perks' dice: an integer 0 to 99 a roll. `forced` is read first (a
/// test's).
#[derive(Resource, Debug)]
pub struct CaPerkRng {
    state: u64,
    pub forced: Vec<u32>,
}

impl Default for CaPerkRng {
    fn default() -> Self {
        Self {
            state: 0x9e37_79b9_7f4a_7c15,
            forced: Vec::new(),
        }
    }
}

impl CaPerkRng {
    /// The next roll, 0 to 99.
    pub fn roll(&mut self) -> u32 {
        if !self.forced.is_empty() {
            return self.forced.remove(0);
        }
        // splitmix64
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) % 100) as u32
    }
}

/// One working perk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivePerk {
    pub ty: i32,
    /// 0 basic, 1 Expert.
    pub level: u8,
}

/// The abilities working on a player now, in the order equipped.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CaPerks {
    pub list: Vec<ActivePerk>,
}

impl CaPerks {
    /// The level of perk `ty`, if it works.
    pub fn level(&self, ty: i32) -> Option<usize> {
        self.list.iter().find(|p| p.ty == ty).map(|p| p.level as usize)
    }

    pub fn has(&self, ty: i32) -> bool {
        self.level(ty).is_some()
    }
}

/// What the perks change this tick, for the movement, the weapons and the
/// effects (replicated, so a client predicts its own player with it).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CaPerkEffects {
    /// The stamina gauge's extra drain, sp/s.
    pub drain: f32,
    /// No stamina is lost: not sprinting, not a perk (War Cry, Virulent).
    pub free_stamina: bool,
    /// The gauge doesn't recover (Awareness, Steady Hand, Inner Focus).
    pub no_recover: bool,
    /// Victory Lap: the run flag uses the sprint speed limit.
    pub victory_lap: bool,
    /// Steady Hand: the scope doesn't sway.
    pub steady: bool,
    /// Eagle Eye: the spread limits shrink by this fraction.
    pub spread_cut: f32,
    /// Awareness: enemy footsteps heard at this volume and radius factor
    /// (`FootSoundVolume`, `SoundRadius`), plus one more step sound.
    pub awareness: bool,
    pub aware_volume: f32,
    pub aware_radius: f32,
    /// Specter: the player's own footsteps' volume factor, walking and
    /// sprinting.
    pub step_volume: f32,
    pub dash_volume: f32,
}

impl Default for CaPerkEffects {
    fn default() -> Self {
        Self {
            drain: 0.0,
            free_stamina: false,
            no_recover: false,
            victory_lap: false,
            steady: false,
            spread_cut: 0.0,
            awareness: false,
            aware_volume: 1.0,
            aware_radius: 1.0,
            step_volume: 1.0,
            dash_volume: 1.0,
        }
    }
}

/// A perk waiting to start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wait {
    pub ty: i32,
    pub until: f64,
    /// Spoils of War's and Opportunist's target.
    pub target: Option<Entity>,
}

/// A player's perks' state (the server's).
#[derive(Component, Clone, Debug, Default)]
pub struct CaPerkState {
    pub waiting: Option<Wait>,
    /// Cooldowns: the perk's type to the time it is ready again.
    pub cooldown: HashMap<i32, f64>,
    /// Perks used up (Spoils of War after `MaxCount`; Inner Focus until
    /// the next death or round).
    pub used: Vec<i32>,
    /// Awareness is on.
    pub awareness: bool,
    /// Victory Lap: the multi-kill counter, the last kill, the end.
    pub kills: u32,
    pub last_kill: Option<f64>,
    pub victory_until: f64,
    /// War Cry's and Eagle Eye's buffs: the end and who gave it.
    pub war_cry: Option<(f64, Entity)>,
    pub eagle_eye: Option<(f64, Entity)>,
    /// Spoils of War: pickups so far, and the GP they earned.
    pub spoils: u32,
    pub gp: u32,
    /// The Pitcher bar, 0 to 100, and last tick's fire.
    pub pitcher: f32,
    fire_held: bool,
    /// The round counter last seen (Inner Focus is ready again).
    round: u32,
}

impl CaPerkState {
    pub fn ready(&self, ty: i32, now: f64) -> bool {
        !self.used.contains(&ty) && self.cooldown.get(&ty).is_none_or(|&t| now >= t)
    }
}

/// A client asks to start the perk at HUD position `slot` (1 to 4).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaPerkRequest {
    pub slot: u8,
}

// ---------------------------------------------------------------------------
// Data

/// The perk record `ty` of the gear data.
fn record(data: &GearData, ty: i32) -> Option<&PerkRecord> {
    data.perks.iter().find(|p| p.ty == ty)
}

fn value(data: &GearData, ty: i32, key: &str, level: usize) -> f32 {
    record(data, ty).map_or(0.0, |r| r.value(key, level))
}

/// The perks working for a loadout in a mode (section 13): the working
/// ones in the order equipped.
pub fn working(data: &GearData, loadout: &CaLoadout, mode: &str, enabled: bool, human: bool) -> Vec<ActivePerk> {
    if !enabled || !human {
        return Vec::new();
    }
    let mut out: Vec<ActivePerk> = Vec::new();
    for &item in &loadout.perks {
        let Some(p) = data.perks.iter().find(|p| p.level_of(item).is_some()) else { continue };
        let level = p.level_of(item).unwrap_or(0) as u8;
        if out.iter().any(|a| a.ty == p.ty) {
            continue;
        }
        let allowed = match p.ty {
            RELENTLESS => RELENTLESS_MODES.contains(&mode),
            PARTING_GIFT => mode != super::gear::QUARANTINE,
            SPOILS_OF_WAR | OPPORTUNIST => mode != "VWRM",
            _ => true,
        };
        if allowed {
            out.push(ActivePerk { ty: p.ty, level });
        }
    }
    out
}

/// The respawn wait Relentless gives (14): `RespawnTime` s, None without
/// it. The death camera's `CaRelentless` is this.
pub fn respawn_wait(data: &GearData, perks: &CaPerks) -> Option<f32> {
    perks.level(RELENTLESS).map(|l| value(data, RELENTLESS, "RespawnTime", l))
}

/// A weapon's `Guntype` (weapon_classes.md): the install's record, else
/// our built-in weapons'.
pub fn gun_type(data: &super::data::CaDataInner, id: &str) -> Option<i32> {
    // `install_weapon` wants the namespaced id (it strips `combat_arms:`
    // itself), the built-in table the short one.
    if let Some(r) = data.install_weapon(id) {
        return Some(r.gun_type);
    }
    let short = id.strip_prefix("combat_arms:").unwrap_or(id);
    Some(match short {
        "m9" => 0,
        "beretta_m92fs" => 1,
        "remington_870" => 2,
        "mp5a4" => 3,
        "m4a1" => 4,
        "m60" => 5,
        "l96a1" | "dragunov_svd" => 6,
        "m67" | "xm84" | "m18" | "m23f" | "g7a" => 7,
        _ => return None,
    })
}

/// Guntype of a sniper rifle.
pub const SNIPER: i32 = 6;
/// Guntype of a thrown weapon.
pub const THROWN: i32 = 7;

/// Resourceful's weapons: rifle, sniper, pistol, SMG, shotgun, knife and
/// thrown.
fn resourceful_weapon(ty: i32) -> bool {
    matches!(ty, 0..=4 | SNIPER | THROWN)
}

/// Warlord's charge time by gun type, s (14); None: cannot charge.
pub fn charge_time(ty: i32) -> Option<f32> {
    Some(match ty {
        0 => 1.0,
        1..=3 => 2.0,
        4 => 3.0,
        5 => 2.0,
        SNIPER => 4.0,
        THROWN => 2.0,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// The plugin

/// Combat Arms' perks.
pub struct CaPerksPlugin;

impl Plugin for CaPerksPlugin {
    fn build(&self, app: &mut App) {
        super::data::CaData::of(app);
        app.init_resource::<CaPerksEnabled>()
            .init_resource::<CaPerkRng>()
            .add_message::<Damage>()
            .net_part("combat_arms_perks", register)
            .add_systems(
                FixedUpdate,
                (
                    derive_perks.after(SimSet::Weapons),
                    wire_respawn.run_if(core::authoritative).after(derive_perks),
                    spread_cuts.before(SimSet::Weapons),
                    (ensure_state, perk_tick)
                        .chain()
                        .run_if(core::authoritative)
                        .after(SimSet::Commands)
                        .before(SimSet::Movement),
                    (perk_deaths, supplies)
                        .chain()
                        .run_if(core::authoritative)
                        .after(core::apply_damage),
                ),
            );
        crate::console::resource_cvar::<CaPerksEnabled, bool>(app, "ca_perks_enabled", "The room option Perks.", |p| &mut p.0);
        app.console_command(
            "ca_perk",
            "ca_perk <1-4>: start the active perk in that position (the use-perk key plus a number).",
            |w, a| {
                let slot: u8 = a.first().and_then(|s| s.parse().ok()).filter(|s| (1..=4).contains(s)).ok_or("ca_perk <1-4>")?;
                let me = w
                    .query_filtered::<Entity, With<LocalPlayer>>()
                    .iter(w)
                    .next()
                    .ok_or("no local player")?;
                if w.get_resource::<NetRole>().is_some_and(|r| *r == NetRole::Client) {
                    w.write_message(CaPerkRequest { slot });
                    return Ok(Some("perk requested".into()));
                }
                use_perk(w, me, slot as usize).map(Some)
            },
        );
    }
}

fn register(app: &mut App) {
    app.replicate::<CaPerkEffects>()
        .ordered::<CaPerkEffects>()
        .add_client_message::<CaPerkRequest>(Channel::Ordered)
        .add_systems(
            PreUpdate,
            receive_requests
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        );
}

/// A client's request, for its player.
fn receive_requests(
    mut requests: MessageReader<FromClient<CaPerkRequest>>,
    players: Query<&Player>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, slot) = (p.character, r.message.slot as usize);
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_ok()
                && let Err(why) = use_perk(w, character, slot)
            {
                debug!("perk request refused: {why}");
            }
        });
    }
}

/// Every side: the perks working on each character.
#[allow(clippy::type_complexity)]
fn derive_perks(
    data: Res<CaGearData>,
    view: Res<CaMatchView>,
    enabled: Res<CaPerksEnabled>,
    characters: Query<(Entity, Option<&CaLoadout>, Option<&CaPerks>, Has<CaInfected>, Has<CaVip>), (With<Intent>, With<MovementState>)>,
    mut commands: Commands,
) {
    for (e, loadout, have, infected, vip) in &characters {
        let list = match loadout {
            Some(l) => working(&data.0, l, &view.mode, enabled.0, !infected && !vip),
            None => Vec::new(),
        };
        let want = CaPerks { list };
        if have != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// The server: a player with perks has their state, and a tick's effects.
fn ensure_state(
    players: Query<(Entity, &CaPerks, Option<&CaPerkState>, Option<&CaPerkEffects>, Has<CaInfected>, Option<&CaLoadout>)>,
    mut commands: Commands,
) {
    for (e, perks, state, effects, infected, loadout) in &players {
        // Virulent (15) is an infected form's with the item.
        let wired = infected && loadout.is_some_and(|l| l.infected_item);
        let any = !perks.list.is_empty() || wired;
        if state.is_none() && any {
            commands.entity(e).insert(CaPerkState::default());
        }
        if effects.is_none() && (state.is_some() || any) {
            commands.entity(e).insert(CaPerkEffects::default());
        }
    }
}

// ---------------------------------------------------------------------------
// Starting a perk

/// The nearest dropped weapon to `from` within `range` (u), if any.
fn nearest_drop(world: &mut World, from: Vec3, range: f32) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;
    let mut q = world.query_filtered::<(Entity, &Transform), With<Dropped>>();
    for (e, t) in q.iter(world) {
        let d = t.translation.distance(from) / UNIT;
        if d <= range && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, e));
        }
    }
    best.map(|(_, e)| e)
}

/// The nearest dead body within `range` (u) that still carries backpack
/// primaries.
fn nearest_body(world: &mut World, me: Entity, from: Vec3, range: f32) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;
    let mut q = world.query_filtered::<(Entity, &Transform), (With<Dead>, With<Inventory>, Without<CaLooted>)>();
    for (e, t) in q.iter(world) {
        if e == me {
            continue;
        }
        let d = t.translation.distance(from) / UNIT;
        if d <= range && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, e));
        }
    }
    best.map(|(_, e)| e)
}

/// A body whose backpack was looted (Opportunist).
#[derive(Component)]
pub struct CaLooted;

/// A client's or the console's use-perk request for the perk at HUD
/// position `slot` (from 1). The server checks the perk's local
/// conditions, then starts it or waits (13).
pub fn use_perk(world: &mut World, e: Entity, slot: usize) -> Result<String, String> {
    let data = world.resource::<CaGearData>().0.clone();
    let now = world.resource::<SimClock>().now;
    let perks = world.get::<CaPerks>(e).cloned().unwrap_or_default();
    let active = *perks.list.get(slot.wrapping_sub(1)).ok_or("no perk in that position")?;
    let rec = record(&data, active.ty).ok_or("unknown perk")?;
    if rec.kind != PerkKind::Active {
        return Err(format!("{} is always on", rec.name));
    }
    if world.get::<Dead>(e).is_some() {
        return Err("dead".into());
    }
    let mut state = world.get::<CaPerkState>(e).cloned().ok_or("no perk state")?;
    if state.waiting.is_some() {
        return Err("already waiting".into());
    }
    if !state.ready(active.ty, now) {
        return Err(format!("{} is not ready", rec.name));
    }
    if state.awareness && active.ty == AWARENESS {
        return Err("already on".into());
    }
    let level = active.level as usize;
    let at = world.get::<Transform>(e).map(|t| t.translation).unwrap_or_default();
    let mut target = None;
    match active.ty {
        AWARENESS => {
            if world.get::<CaMovement>(e).is_none_or(|m| m.stamina < 1.0) {
                return Err("no stamina".into());
            }
        }
        INNER_FOCUS => {
            if world.get::<Health>(e).is_none_or(|h| h.current >= h.max) {
                return Err("full HP".into());
            }
        }
        SPOILS_OF_WAR => {
            let range = value(&data, SPOILS_OF_WAR, "TargetDistance", level);
            target = Some(nearest_drop(world, at, range).ok_or("nothing to take")?);
        }
        OPPORTUNIST => {
            let range = value(&data, OPPORTUNIST, "TargetDistance", level);
            target = Some(nearest_body(world, e, at, range).ok_or("no body in reach")?);
        }
        _ => {}
    }
    let wait = value(&data, active.ty, "WaitTime", level) as f64;
    if wait > 0.0 {
        state.waiting = Some(Wait {
            ty: active.ty,
            until: now + wait,
            target,
        });
        world.entity_mut(e).insert(state);
        return Ok(format!("{} starts in {wait} s", rec.name));
    }
    world.entity_mut(e).insert(state);
    begin(world, &data, e, active, target, now)?;
    Ok(format!("{} started", rec.name))
}

/// The perk begins (the wait ended or there was none).
fn begin(world: &mut World, data: &GearData, e: Entity, active: ActivePerk, target: Option<Entity>, now: f64) -> Result<(), String> {
    let level = active.level as usize;
    let cool = value(data, active.ty, "CoolTime", level) as f64;
    let mut ok = true;
    match active.ty {
        AWARENESS => {
            if let Some(mut s) = world.get_mut::<CaPerkState>(e) {
                s.awareness = true;
            }
        }
        INNER_FOCUS => {
            let heal = value(data, INNER_FOCUS, "HealHP", level) / 100.0;
            if let Some(mut h) = world.get_mut::<Health>(e) {
                h.current = (h.current + heal).min(h.max);
            }
            if let Some(mut m) = world.get_mut::<CaMovement>(e) {
                m.stamina = 0.0;
            }
            if let Some(mut s) = world.get_mut::<CaPerkState>(e) {
                s.used.push(INNER_FOCUS);
            }
        }
        SPOILS_OF_WAR => {
            let range = value(data, SPOILS_OF_WAR, "TargetDistance", level);
            let at = world.get::<Transform>(e).map(|t| t.translation).unwrap_or_default();
            // The server re-checks the distance (3D, 250 u).
            let near = target.is_some_and(|t| {
                world
                    .get::<Transform>(t)
                    .is_some_and(|tt| tt.translation.distance(at) / UNIT <= range)
            });
            if near {
                if let Some(t) = target
                    && let Some(d) = world.get::<Dropped>(t).copied()
                {
                    world.despawn(t);
                    if world.get::<Weapon>(d.weapon).is_some_and(|w| w.owner.is_none()) {
                        world.despawn(d.weapon);
                    }
                }
                let add = value(data, SPOILS_OF_WAR, "AddGP", level) as u32;
                let max = value(data, SPOILS_OF_WAR, "MaxCount", level) as u32;
                if let Some(mut s) = world.get_mut::<CaPerkState>(e) {
                    s.gp += add;
                    s.spoils += 1;
                    if s.spoils >= max {
                        s.used.push(SPOILS_OF_WAR);
                    }
                }
            } else {
                ok = false;
            }
        }
        OPPORTUNIST => {
            if let Some(body) = target {
                ok = loot(world, data, level, e, body);
            }
        }
        WARLORD => place_supply(world, data, e, level, now),
        WAR_CRY | EAGLE_EYE => buff(world, data, e, active, now),
        _ => {}
    }
    if ok && cool > 0.0 {
        if let Some(mut s) = world.get_mut::<CaPerkState>(e) {
            s.cooldown.insert(active.ty, now + cool);
        }
    }
    Ok(())
}

/// War Cry or Eagle Eye: the user and every teammate strictly closer than
/// `TargetDistance` get the buff for `Time` s; a new one replaces the old
/// timer (13).
fn buff(world: &mut World, data: &GearData, e: Entity, active: ActivePerk, now: f64) {
    let level = active.level as usize;
    let range = value(data, active.ty, "TargetDistance", level);
    let time = value(data, active.ty, "Time", level) as f64;
    let team = world.get::<Team>(e).map(|t| t.0);
    let at = world.get::<Transform>(e).map(|t| t.translation).unwrap_or_default();
    let mut receivers = vec![e];
    let mut q = world.query_filtered::<(Entity, &Transform, Option<&Team>, Has<Dead>), With<Intent>>();
    for (o, t, tm, _dead) in q.iter(world) {
        if o != e && tm.map(|t| t.0) == team && t.translation.distance(at) / UNIT < range {
            receivers.push(o);
        }
    }
    for r in receivers {
        let mut ent = world.entity_mut(r);
        if ent.get::<CaPerkState>().is_none() {
            ent.insert(CaPerkState::default());
        }
        if ent.get::<CaPerkEffects>().is_none() {
            ent.insert(CaPerkEffects::default());
        }
        if let Some(mut s) = ent.get_mut::<CaPerkState>() {
            let slot = if active.ty == WAR_CRY { &mut s.war_cry } else { &mut s.eagle_eye };
            *slot = Some((now + time, e));
        }
    }
}

/// Opportunist: the body's carried primaries other than the one it
/// dropped are dropped again at its torso (the hull centre) +-20 u in x
/// and z with a random yaw, reserved for the looter for `OwnerTime` s and
/// then for the looter's team, up to `DropWeaponMax`; the body is removed
/// (`CaBody`; its character is marked looted). A body with none fails
/// ("empty backpack") and stays (Corrections 2026-10-22, 7). Returns
/// whether it worked.
fn loot(world: &mut World, data: &GearData, level: usize, looter: Entity, body: Entity) -> bool {
    let max = value(data, OPPORTUNIST, "DropWeaponMax", level).max(1.0) as usize;
    let owner_time = value(data, OPPORTUNIST, "OwnerTime", level) as f64;
    let now = world.resource::<SimClock>().now;
    let carried = world.get::<Inventory>(body).map(|i| i.weapons.clone()).unwrap_or_default();
    let last = world.get::<inventory::CaSelection>(body).and_then(|s| s.last_chosen[inventory::PRIMARY as usize]);
    let backpack: Vec<Entity> = carried
        .into_iter()
        .filter(|&w| {
            world.get::<Weapon>(w).is_some_and(|x| inventory::class_of(x.slot) == inventory::PRIMARY) && Some(w) != last
        })
        .take(max)
        .collect();
    if backpack.is_empty() {
        return false;
    }
    let torso = world.get::<Transform>(body).map(|t| t.translation).unwrap_or_default();
    let team = world.get::<Team>(looter).map(|t| t.0);
    for (i, w) in backpack.into_iter().enumerate() {
        inventory::drop_item(world, body, w, false);
        let mut rng = shared_random((now * 1000.0) as u32, "combat_arms:loot", i as i32);
        let spot = torso + Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)) * LOOT_SPREAD * UNIT;
        let yaw = rng.range(0.0, 1.0);
        let item = world
            .query::<(Entity, &Dropped)>()
            .iter(world)
            .find(|(_, d)| d.weapon == w)
            .map(|(e, _)| e);
        if let Some(item) = item {
            world.entity_mut(item).insert((
                Transform::from_translation(spot).with_rotation(Quat::from_rotation_y(yaw)),
                inventory::CaReserved {
                    owner: looter,
                    team,
                    until: now + owner_time,
                },
            ));
        }
    }
    let bodies: Vec<Entity> = world
        .query::<(Entity, &super::characters::CaBody)>()
        .iter(world)
        .filter(|(_, b)| b.owner == body)
        .map(|(e, _)| e)
        .collect();
    for b in bodies {
        world.despawn(b);
    }
    world.entity_mut(body).insert(CaLooted);
    true
}

// ---------------------------------------------------------------------------
// The tick

/// Whether the intent holds a movement key (forward, back, strafe, jump).
fn moving(i: &Intent) -> bool {
    i.move_axis != Vec2::ZERO || i.opposed.any() || i.jump
}

/// Each server tick, before the movement: waits, running perks, buffs,
/// the Pitcher bar and the effects they come to.
fn perk_tick(world: &mut World) {
    track_last_primary(world);
    let now = world.resource::<SimClock>().now;
    let dt = world.resource::<SimClock>().dt();
    let rounds = world.get_resource::<RoundRestarts>().map_or(0, |r| r.0);
    let data = world.resource::<CaGearData>().0.clone();
    let players: Vec<Entity> = cached_query::<Entity, With<CaPerkState>, _>(world, |q, w| q.iter(w).collect());
    for e in players {
        tick_one(world, &data, e, now, dt, rounds);
    }
}

fn tick_one(world: &mut World, data: &GearData, e: Entity, now: f64, dt: f32, rounds: u32) {
    let perks = world.get::<CaPerks>(e).cloned().unwrap_or_default();
    let dead = world.get::<Dead>(e).is_some();
    let intent = world.get::<Intent>(e).cloned().unwrap_or_default();
    let stamina = world.get::<CaMovement>(e).map_or(0.0, |m| m.stamina);
    let sprinting = world.get::<MovementState>(e).is_some_and(|s| s.sprinting);
    let zoomed_sniper = active_weapon(world, e).is_some_and(|w| {
        world.get::<CaZoom>(w).is_some_and(|z| z.zoomed()) && world.get::<Weapon>(w).is_some_and(|x| gun_type(world.resource::<super::data::CaData>(), x.id) == Some(SNIPER))
    });
    let thrown_in_hand = active_weapon(world, e).is_some_and(|w| world.get::<Throwable>(w).is_some());
    let giver_gone = |w: &World, g: Entity| w.get_entity(g).is_err();
    // A wait, then a running perk.
    let mut to_begin = None;
    {
        let mut s = world.get_mut::<CaPerkState>(e).unwrap();
        if dead {
            s.waiting = None;
            s.awareness = false;
            // Inner Focus is ready again on death (13).
            s.used.retain(|&t| t != INNER_FOCUS);
        }
        if s.round != rounds {
            s.round = rounds;
            s.used.retain(|&t| t != INNER_FOCUS);
        }
        if let Some(w) = s.waiting {
            if moving(&intent) {
                s.waiting = None;
            } else if now >= w.until {
                s.waiting = None;
                to_begin = Some(w);
            }
        }
        if s.awareness && (moving(&intent) || stamina < 1.0) {
            s.awareness = false;
        }
        let CaPerkState { war_cry, eagle_eye, .. } = &mut *s;
        for slot in [war_cry, eagle_eye] {
            if slot.is_some_and(|(until, _)| now >= until) {
                *slot = None;
            }
        }
    }
    if let Some(w) = to_begin
        && let Some(active) = perks.list.iter().find(|p| p.ty == w.ty).copied()
    {
        let _ = begin(world, data, e, active, w.target, now);
    }
    // Buffs end at once when the giver leaves.
    let gone = {
        let s = world.get::<CaPerkState>(e).unwrap();
        (s.war_cry.is_some_and(|(_, g)| giver_gone(world, g)), s.eagle_eye.is_some_and(|(_, g)| giver_gone(world, g)))
    };
    let pitcher_per = perks.level(PITCHER).map(|l| value(data, PITCHER, "BarRightSpeed", l) * 10.0 / 1000.0);
    let (awareness, war_cry, eagle, used_inner, victory_until) = {
        let mut s = world.get_mut::<CaPerkState>(e).unwrap();
        if gone.0 {
            s.war_cry = None;
        }
        if gone.1 {
            s.eagle_eye = None;
        }
        // The Pitcher bar (14): one unit per `BarRightSpeed` x 10 ms.
        if let Some(per) = pitcher_per {
            let held = intent.fire && thrown_in_hand;
            if held && !s.fire_held {
                s.pitcher = 0.0;
            }
            if held && !sprinting && per > 0.0 {
                s.pitcher = (s.pitcher + dt / per).min(100.0);
            }
            s.fire_held = held;
        }
        (s.awareness, s.war_cry.is_some(), s.eagle_eye.is_some(), s.used.contains(&INNER_FOCUS), s.victory_until)
    };
    // The effects.
    let virulent = world.get::<CaInfected>(e).is_some() && world.get::<CaLoadout>(e).is_some_and(|l| l.infected_item);
    let mut fx = CaPerkEffects::default();
    if !dead {
        let steady = perks.level(STEADY_HAND).filter(|_| zoomed_sniper && stamina >= 1.0);
        if let Some(l) = steady {
            fx.steady = true;
            fx.drain += value(data, STEADY_HAND, "SPDecrease", l);
            fx.no_recover = true;
        }
        if awareness
            && let Some(l) = perks.level(AWARENESS)
        {
            fx.awareness = true;
            fx.aware_volume = value(data, AWARENESS, "FootSoundVolume", l);
            fx.aware_radius = value(data, AWARENESS, "SoundRadius", l);
            fx.drain += value(data, AWARENESS, "SPDecrease", l);
            fx.no_recover = true;
        }
        if used_inner {
            fx.no_recover = true;
        }
        if war_cry {
            fx.free_stamina = true;
            fx.drain = 0.0;
        }
        if eagle {
            fx.spread_cut = value(data, EAGLE_EYE, "Purturb", 0);
        }
        fx.victory_lap = now < victory_until && perks.has(VICTORY_LAP);
        // Virulent (15): an infected form with an infected-mercenary item
        // sprints free.
        if virulent {
            fx.free_stamina = true;
            fx.drain = 0.0;
        }
    }
    if let Some(l) = perks.level(SPECTER) {
        fx.step_volume = value(data, SPECTER, "FootSoundVolume", l);
        fx.dash_volume = value(data, SPECTER, "DashSoundVolume", l);
    }
    if world.get::<CaPerkEffects>(e) != Some(&fx) {
        world.entity_mut(e).insert(fx);
    }
}

/// A player's weapon in hand.
fn active_weapon(world: &World, e: Entity) -> Option<Entity> {
    world.get::<Inventory>(e).and_then(|i| i.active)
}

// ---------------------------------------------------------------------------
// Deaths

/// Whether `a` and `b` (teams `ta`, `tb`) are enemies in `mode` now.
fn enemies(mode: &str, a: Entity, b: Entity, ta: Option<u8>, tb: Option<u8>) -> bool {
    a != b && (EVERYONE_ENEMIES.contains(&mode) || ta != tb)
}

/// What `perk_deaths` reads of the characters.
type Fighters<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Team>,
        Option<&'static CaPerks>,
        Option<&'static Health>,
        Option<&'static Inventory>,
        Option<&'static Transform>,
    ),
>;

/// What a kill does for the killer's perks and the victim's: a normal
/// system reading the tick's `Died` (plan ecs-cleanup step 5). Victory
/// Lap's counter is written in place and Resourceful's roll made here, in
/// the deaths' order as before; the boxes and Parting Gift's grenade
/// (which builds a weapon: world work) go through `Commands`, applied at
/// the sync point before `supplies`, which is chained after.
#[allow(clippy::too_many_arguments)]
fn perk_deaths(
    mut died: MessageReader<Died>,
    gear: Res<CaGearData>,
    clock: Res<SimClock>,
    view: Res<CaMatchView>,
    ca_data: Res<super::data::CaData>,
    mut rng: ResMut<CaPerkRng>,
    fighters: Fighters,
    mut states: Query<&mut CaPerkState>,
    weapons: Query<(Option<&Weapon>, Option<&Magazine>, Option<&Throwable>)>,
    mut commands: Commands,
) {
    let data: &GearData = &gear.0;
    let now = clock.now;
    for d in died.read() {
        let victim = d.entity;
        let team = |e: Entity| fighters.get(e).ok().and_then(|f| f.0).map(|t| t.0);
        if let Some(killer) = d.attacker.filter(|&k| enemies(&view.mode, k, victim, team(k), team(victim))) {
            let perks = fighters.get(killer).ok().and_then(|f| f.1).cloned().unwrap_or_default();
            if let Some(l) = perks.level(VICTORY_LAP) {
                let start = value(data, VICTORY_LAP, "StartKill", l) as u32;
                let time = value(data, VICTORY_LAP, "Time", l) as f64;
                if let Ok(mut s) = states.get_mut(killer) {
                    s.kills = match s.last_kill {
                        // Strictly under 5000 ms (Corrections 2026-10-22, 10).
                        Some(t) if ((now - t) * 1000.0).round() < MULTI_KILL_WINDOW * 1000.0 => s.kills + 1,
                        _ => 1,
                    };
                    s.last_kill = Some(now);
                    if s.kills >= start {
                        s.victory_until = now + time;
                    }
                }
            }
            if let Some(l) = perks.level(RESOURCEFUL)
                && !view.warmup
                && let Some(kind) = resourceful(data, &ca_data, &fighters, &weapons, &mut rng, killer, d, l)
            {
                let at = fighters.get(victim).ok().and_then(|f| f.4).map(|t| t.translation).unwrap_or_default();
                commands.spawn((
                    Name::new("Perk box"),
                    Transform::from_translation(at),
                    CaPerkBox {
                        kind,
                        owner: killer,
                        team: team(killer).unwrap_or(0),
                        since: now,
                        hide: value(data, RESOURCEFUL, "BoxHideTime", l) as f64,
                        life: value(data, RESOURCEFUL, "BoxLifeTime", l) as f64,
                        heal: value(data, RESOURCEFUL, "HealthDrop_HP", l),
                    },
                ));
            }
        }
        let perks = fighters.get(victim).ok().and_then(|f| f.1).cloned().unwrap_or_default();
        if let Some(l) = perks.level(PARTING_GIFT) {
            let gear = gear.0.clone();
            commands.queue(move |world: &mut World| parting_gift(world, &gear, victim, l));
        }
    }
}

/// Resourceful (14): a kill with a gun-type weapon rolls 0 to 99 against
/// `DropProbability`; the box is a medikit when the killer's health is
/// below `HealthDrop_HP`, else an ammo box when the held weapon's ammo
/// percentage is at most `AmmoDrop_Ammo` (a full-ammo box when a thrown
/// weapon is out), else nothing.
#[allow(clippy::too_many_arguments)]
fn resourceful(
    data: &GearData,
    ca_data: &super::data::CaData,
    fighters: &Fighters,
    weapons: &Query<(Option<&Weapon>, Option<&Magazine>, Option<&Throwable>)>,
    rng: &mut CaPerkRng,
    killer: Entity,
    d: &Died,
    level: usize,
) -> Option<BoxKind> {
    let (_, _, health, inventory, _) = fighters.get(killer).ok()?;
    let held = inventory.and_then(|i| i.active);
    let part = |w: Entity| weapons.get(w).ok();
    let weapon_id = d.damage.weapon.or_else(|| held.and_then(|h| part(h)?.0.map(|w| w.id)));
    if !weapon_id.and_then(|id| gun_type(ca_data, id)).is_some_and(resourceful_weapon) {
        return None;
    }
    let probability = value(data, RESOURCEFUL, "DropProbability", level) as u32;
    if rng.roll() >= probability {
        return None;
    }
    let hp = health.map_or(100.0, |h| h.current * 100.0);
    let hp_limit = value(data, RESOURCEFUL, "HealthDrop_HP", level);
    let ammo_limit = value(data, RESOURCEFUL, "AmmoDrop_Ammo", level);
    // A thrown weapon in hand counts by its own count over its maximum,
    // so a spent grenade is 0 % (Corrections 2026-10-22, 3).
    let percent = held.and_then(part).and_then(|(_, m, t)| {
        m.map(|m| {
            let max = (m.reserve_max + m.size).max(1);
            ((100 * (m.clip + m.reserve)) / max) as f32
        })
        .or_else(|| t.map(|t| ((100 * t.count) / t.max.max(1)) as f32))
    });
    let empty_thrown = inventory
        .is_some_and(|i| i.weapons.iter().any(|w| part(*w).and_then(|p| p.2).is_some_and(|t| t.count == 0)));
    if hp < hp_limit {
        Some(BoxKind::Medikit)
    } else if percent.is_some_and(|p| p <= ammo_limit) {
        Some(if empty_thrown { BoxKind::FullAmmo } else { BoxKind::Ammo })
    } else {
        None
    }
}

/// The fuse of the perk's grenade, s (Corrections 2026-10-22, 8).
pub const PARTING_GIFT_FUSE: f32 = 2.5;

/// Parting Gift (14): the perk's own frag (the stock frag's data; the
/// perk's `WeaponID` records are not read) made at the body, going
/// straight up at its `DropVelocity` on a fixed 2.5 s fuse, credited to
/// the dead, whatever the dead carried (Corrections 2026-10-22, 8).
fn parting_gift(world: &mut World, data: &GearData, victim: Entity, level: usize) {
    let area = if level == 0 { 20.0 } else { 24.0 };
    let _ = data;
    let at = world.get::<Transform>(victim).map(|t| t.translation).unwrap_or_default();
    let Some(template) = crate::weapon::spawn_weapon(world, grenades::M67, victim) else { return };
    let drop = world
        .get::<grenades::CaGrenadeData>(template)
        .map_or(0.0, |d| d.params.drop_speed * d.unit);
    if let Some(g) = grenades::spawn_grenade(world, template, Some(victim), at, Vec3::Y * drop)
        && let Some(mut c) = world.get_mut::<grenades::CaGrenade>(g)
    {
        c.params.area_damage = area;
        c.params.fuse = PARTING_GIFT_FUSE;
    }
    world.despawn(template);
}

// ---------------------------------------------------------------------------
// Supplies

/// What a Resourceful box refills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Medikit,
    Ammo,
    FullAmmo,
}

/// A Resourceful box: at the victim's place, `life` s long; its killer
/// alone can take it for the first `hide` s, then its team.
#[derive(Component, Clone, Copy, Debug)]
pub struct CaPerkBox {
    pub kind: BoxKind,
    pub owner: Entity,
    pub team: u8,
    pub since: f64,
    pub hide: f64,
    pub life: f64,
    /// A medikit's heal, hit points.
    pub heal: f32,
}

/// A charge in progress at a supply point.
#[derive(Clone, Copy, Debug)]
pub struct Charge {
    pub player: Entity,
    pub weapon: Entity,
    pub gun_type: i32,
    pub started: f64,
    pub at: Vec3,
    pub time: f32,
}

/// Warlord's supply point (14): `charges` left (one is reserved while a
/// charge runs).
#[derive(Component, Clone, Debug)]
pub struct CaSupplyBox {
    pub owner: Entity,
    pub charges: u32,
    pub busy: Vec<Charge>,
    /// Round counter at creation: a new round removes it.
    pub round: u32,
}

/// Warlord begins: a supply point ahead of the owner, replacing their old
/// one.
fn place_supply(world: &mut World, data: &GearData, e: Entity, level: usize, _now: f64) {
    let old: Vec<Entity> = world
        .query::<(Entity, &CaSupplyBox)>()
        .iter(world)
        .filter(|(_, b)| b.owner == e)
        .map(|(b, _)| b)
        .collect();
    for b in old {
        world.despawn(b);
    }
    let Some(t) = world.get::<Transform>(e).copied() else { return };
    let yaw = world.get::<Intent>(e).map_or(0.0, |i| i.yaw);
    let ahead = Quat::from_rotation_y(yaw) * Vec3::NEG_Z * SUPPLY_AHEAD * UNIT;
    // At the thrower's floor level.
    let feet = world.get::<MovementState>(e).map_or(0.0, |s| s.hull_min.y);
    let ahead = ahead + Vec3::Y * feet;
    let charges = value(data, WARLORD, "RechargeCount", level).max(1.0) as u32;
    let round = world.get_resource::<RoundRestarts>().map_or(0, |r| r.0);
    world.spawn((
        Name::new("Supply point"),
        Transform::from_translation(t.translation + ahead),
        CaSupplyBox {
            owner: e,
            charges,
            busy: Vec::new(),
            round,
        },
    ));
}

/// A weapon's total and its maximum.
fn ammo_of(m: &Magazine) -> (u32, u32) {
    (m.clip + m.reserve, m.reserve_max + m.size)
}

/// The boxes' frame: expiry and pickups, the supply points' charges.
fn supplies(world: &mut World) {
    let now = world.resource::<SimClock>().now;
    let rounds = world.get_resource::<RoundRestarts>().map_or(0, |r| r.0);
    // Resourceful boxes.
    let boxes: Vec<(Entity, CaPerkBox, Vec3)> = cached_query::<(Entity, &CaPerkBox, &Transform), (), _>(world, |q, w| {
        q.iter(w).map(|(e, b, t)| (e, *b, t.translation)).collect()
    });
    let players: Vec<(Entity, Vec3, u8)> =
        cached_query::<(Entity, &Transform, Option<&Team>), (With<Intent>, Without<Dead>), _>(world, |q, w| {
            q.iter(w).map(|(e, t, team)| (e, t.translation, team.map_or(0, |t| t.0))).collect()
        });
    for (b, bx, at) in boxes {
        if now - bx.since >= bx.life {
            world.despawn(b);
            continue;
        }
        let alone = now - bx.since < bx.hide;
        // The first player in reach who can use it takes it; a box nobody
        // can use stays (Corrections 2026-10-22, 2 to 4).
        let candidates: Vec<Entity> = players
            .iter()
            .filter(|(p, pos, team)| {
                pos.distance(at) / UNIT <= PICKUP_RANGE && if alone { *p == bx.owner } else { *team == bx.team }
            })
            .map(|(p, ..)| *p)
            .collect();
        if candidates.into_iter().any(|p| take_box(world, p, &bx)) {
            world.despawn(b);
        }
    }
    // Supply points.
    let points: Vec<Entity> = cached_query::<Entity, With<CaSupplyBox>, _>(world, |q, w| q.iter(w).collect());
    for point in points {
        supply_frame(world, point, now, rounds, &players);
    }
}

/// A weapon's `MaxAmount` from its record: its maximum without the gear's
/// raise (Corrections 2026-10-22, 3).
fn record_max(world: &World, w: Entity) -> Option<u32> {
    let m = world.get::<Magazine>(w)?;
    let raised = world.get::<super::gear::CaAmmoRaised>(w).map_or(0, |r| r.0);
    Some((m.reserve_max + m.size).saturating_sub(raised))
}

/// A box or supply point refills `w`: its total goes to the player's
/// maximum, only when below the record's `MaxAmount`. Whether it did.
fn refill(world: &mut World, w: Entity) -> bool {
    if let Some(record) = record_max(world, w)
        && let Some(mut m) = world.get_mut::<Magazine>(w)
    {
        let total = m.clip + m.reserve;
        if total >= record {
            return false;
        }
        m.reserve = (m.reserve_max + m.size).saturating_sub(m.clip);
        return true;
    }
    false
}

/// The last weapon switched to whose gun type is SMG, assault rifle,
/// machine gun, sniper or rocket launcher, if still carried (the server's
/// "last primary").
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaLastPrimary(pub Option<Entity>);

/// Gun types that become the last primary (weapon_classes.md).
fn records_as_last_primary(ty: i32) -> bool {
    matches!(ty, 3..=6 | 9)
}

/// Each server tick: the weapon in hand becomes the player's last primary
/// when its gun type says so.
fn track_last_primary(world: &mut World) {
    let held: Vec<(Entity, Entity, Option<Entity>)> =
        cached_query::<(Entity, &Inventory, Option<&CaLastPrimary>), (), _>(world, |q, w| {
            q.iter(w).filter_map(|(e, i, l)| Some((e, i.active?, l.and_then(|l| l.0)))).collect()
        });
    for (e, w, last) in held {
        if last == Some(w) {
            continue;
        }
        let ty = world.get::<Weapon>(w).and_then(|x| gun_type(world.resource::<super::data::CaData>(), x.id));
        if ty.is_some_and(records_as_last_primary) {
            world.entity_mut(e).insert(CaLastPrimary(Some(w)));
        }
    }
}

/// Whether the player takes the box (Corrections 2026-10-22, 2 to 4):
/// false when it gave nothing, and the box stays.
fn take_box(world: &mut World, p: Entity, b: &CaPerkBox) -> bool {
    match b.kind {
        BoxKind::Medikit => {
            let max_ap = super::gear::max_armour(world, p);
            let hp_full = world.get::<Health>(p).is_none_or(|h| h.current >= h.max);
            let ap_full = world.get::<crate::weapon::Armor>(p).is_none_or(|a| super::weapons::points(a) >= max_ap);
            if hp_full && ap_full {
                return false;
            }
            if let Some(mut h) = world.get_mut::<Health>(p) {
                h.current = h.max;
            }
            if let Some(mut a) = world.get_mut::<crate::weapon::Armor>(p) {
                a.amount = a.amount.max(max_ap / super::weapons::HIT_POINTS);
            }
            true
        }
        BoxKind::Ammo => {
            let carried = world.get::<Inventory>(p).map(|i| i.weapons.clone()).unwrap_or_default();
            let Some(w) = world.get::<CaLastPrimary>(p).and_then(|l| l.0).filter(|w| carried.contains(w)) else {
                return false;
            };
            // Assault rifles and SMGs only; machine guns and snipers get
            // nothing.
            let rifle_or_smg = world
                .get::<Weapon>(w)
                .and_then(|x| gun_type(world.resource::<super::data::CaData>(), x.id))
                .is_some_and(|ty| matches!(ty, 3 | 4));
            rifle_or_smg && refill(world, w)
        }
        BoxKind::FullAmmo => {
            let all = world.get::<Inventory>(p).map(|i| i.weapons.clone()).unwrap_or_default();
            let mut any = false;
            for w in all {
                // Mission items (the objective class) are left alone.
                if world.get::<Weapon>(w).is_some_and(|x| inventory::class_of(x.slot) == 5) {
                    continue;
                }
                if world.get::<Magazine>(w).is_some() {
                    any |= refill(world, w);
                } else if let Some(mut t) = world.get_mut::<Throwable>(w)
                    && t.count < t.max
                {
                    t.count = t.max;
                    any = true;
                }
            }
            any
        }
    }
}

fn supply_frame(world: &mut World, point: Entity, now: f64, rounds: u32, players: &[(Entity, Vec3, u8)]) {
    let Some(mut b) = world.get::<CaSupplyBox>(point).cloned() else { return };
    let at = world.get::<Transform>(point).map(|t| t.translation).unwrap_or_default();
    // Gone: its owner left, a new round started, or the round is decided
    // (the end-of-round phase; Corrections 2026-10-22, 6).
    let decided = matches!(
        world.resource::<CaMatchView>().phase,
        Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver
    );
    if world.get_entity(b.owner).is_err() || b.round != rounds || decided {
        world.despawn(point);
        return;
    }
    // Charges in progress.
    let mut still = Vec::new();
    for c in std::mem::take(&mut b.busy) {
        let alive = players.iter().any(|(p, _, _)| *p == c.player) && world.get::<CaInfected>(c.player).is_none();
        let pos = world.get::<Transform>(c.player).map(|t| t.translation);
        let held = active_weapon(world, c.player);
        let using = world.get::<Intent>(c.player).is_some_and(|i| i.use_key);
        // Releasing use, moving, dying or becoming infected: cancelled, the
        // charge back. Another weapon does not cancel (Corrections
        // 2026-10-22, 5).
        let kept = alive && pos == Some(c.at) && using;
        if !kept {
            b.charges += 1;
            continue;
        }
        if now - c.started + 1e-9 >= c.time as f64 {
            // The weapon then in hand is refilled when its gun type is the
            // recorded one; the charge is used either way.
            if let Some(h) = held
                && world.get::<Weapon>(h).and_then(|x| gun_type(world.resource::<super::data::CaData>(), x.id)) == Some(c.gun_type)
            {
                refill(world, h);
            }
            continue;
        }
        still.push(c);
    }
    b.busy = still;
    // New touches.
    let team = players.iter().find(|(p, ..)| *p == b.owner).map(|(.., t)| *t).or_else(|| world.get::<Team>(b.owner).map(|t| t.0));
    for &(p, pos, mine) in players {
        if b.charges == 0
            || b.busy.iter().any(|c| c.player == p)
            || pos.distance(at) / UNIT > SUPPLY_RANGE
            || Some(mine) != team
            || !world.get::<Intent>(p).is_some_and(|i| i.use_key)
        {
            continue;
        }
        if world.get::<CaInfected>(p).is_some() {
            continue;
        }
        let Some(w) = active_weapon(world, p) else { continue };
        let Some(ty) = world.get::<Weapon>(w).and_then(|x| gun_type(world.resource::<super::data::CaData>(), x.id)) else { continue };
        let Some(time) = charge_time(ty) else { continue };
        let Some(m) = world.get::<Magazine>(w) else { continue };
        let (total, max) = ammo_of(m);
        if total >= max {
            continue;
        }
        b.charges -= 1;
        b.busy.push(Charge {
            player: p,
            weapon: w,
            gun_type: ty,
            started: now,
            at: pos,
            time,
        });
    }
    // Removed after the last charge, when none is in progress.
    if b.charges == 0 && b.busy.is_empty() {
        world.despawn(point);
        return;
    }
    world.entity_mut(point).insert(b);
}

// ---------------------------------------------------------------------------
// What other systems read

/// The throw force of a throw by `thrower` (14, Pitcher): `MinForce +
/// (MaxForce - MinForce) g / 100` with the perk, else 1; the bar goes back
/// to 0. The force comes from the server's own bar: nothing the client
/// reports is read.
pub fn throw_force(world: &mut World, thrower: Entity) -> f32 {
    let Some(l) = world.get::<CaPerks>(thrower).and_then(|p| p.level(PITCHER)) else {
        return 1.0;
    };
    let data = world.resource::<CaGearData>().0.clone();
    let g = world.get::<CaPerkState>(thrower).map_or(0.0, |s| s.pitcher);
    if let Some(mut s) = world.get_mut::<CaPerkState>(thrower) {
        s.pitcher = 0.0;
    }
    let (min, max) = (value(&data, PITCHER, "MinForce", l), value(&data, PITCHER, "MaxForce", l));
    min + (max - min) * g / 100.0
}

/// The death camera's inputs (death_killcam.md; the server's, replicated):
/// Relentless gives the client's respawn wait (`CaRelentless`: min(wait,
/// `RespawnTime`)), and the loadout's fast-respawn counts and pass become
/// the player's `CaFastRespawn` when the loadout arrives or changes (the
/// respawn request spends them).
#[allow(clippy::type_complexity)]
fn wire_respawn(
    data: Res<CaGearData>,
    perks: Query<(Entity, &CaPerks, Option<&CaRelentless>), Changed<CaPerks>>,
    loadouts: Query<(Entity, &CaLoadout), Changed<CaLoadout>>,
    mut commands: Commands,
) {
    for (e, p, have) in &perks {
        match (respawn_wait(&data.0, p), have) {
            (Some(wait), Some(h)) if (h.0 - wait).abs() < 1e-6 => {}
            (Some(wait), _) => {
                commands.entity(e).insert(CaRelentless(wait));
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<CaRelentless>();
            }
            (None, None) => {}
        }
    }
    for (e, l) in &loadouts {
        commands.entity(e).insert(CaFastRespawn {
            left: l.fast_left,
            max: l.fast_max,
            premium: l.premium,
        });
    }
}

/// Eagle Eye's and a set's cuts of the cone's limits, on every weapon the
/// owner carries (14, 12), on every side (a client predicts its spread).
fn spread_cuts(
    owners: Query<(&Inventory, Option<&CaPerkEffects>, Option<&super::gear::CaGearEffects>)>,
    mut spreads: Query<&mut super::weapons::CaSpread>,
) {
    for (inv, perk, gear) in &owners {
        let cut = perk.map_or(0.0, |p| p.spread_cut);
        let set = gear.map_or(0.0, |g| g.set.spread);
        for &w in &inv.weapons {
            if let Ok(mut s) = spreads.get_mut(w)
                && (s.perk_cut != cut || s.set_max_cut != set)
            {
                s.perk_cut = cut;
                s.set_max_cut = set;
            }
        }
    }
}
