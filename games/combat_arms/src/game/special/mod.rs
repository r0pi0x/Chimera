//! Combat Arms' special weapons (specs/combat_arms/special_weapons.md;
//! sections below are its): impact projectiles (rockets, launcher shells,
//! flame puffs, the air strike's shells), the flamethrower's fire keys,
//! mines and devices, crossbows. Melee weapons beyond the M9 are data of
//! the knife (`inventory`, data/weapons/*.toml).
//!
//! The weapons are data (data/weapons/, docs/weapon-data.md) on Chimera's
//! weapon frame, with these hooks:
//!
//! - `combat_arms:projectile` (`ProjectileParams`, `CaProjectileData`): a
//!   weapon the frame fires (trigger, magazine, shell-by-shell reload and
//!   all) whose `Fired` rounds become impact projectiles here (`fire`):
//!   from the eye along the view, bent by the cone with the meters at 0
//!   (`grenades::throw_perturb`), at the record's speed with none of the
//!   shooter's (1.1);
//! - `combat_arms:flame` (`FlameParams`, `CaFlame`): the flamethrower's
//!   200 ms pre-fire and loop of eight fire keys 100 ms apart in 734 ms
//!   (4), which paces the frame's trigger (predicted with the weapon);
//! - `combat_arms:device` (`DeviceParams`, `CaDeviceData`): a thrown or
//!   placed device (6) on Chimera's `Throwable` (pre-fire, the key,
//!   post-fire, redraw: 6.12; the spent weapon stays at total 0 as the
//!   grenades do), whose launch (`devices::launch`) makes the object;
//! - `combat_arms:bolt` (`BoltParams`, `CaBoltData`): a crossbow whose
//!   rounds become bolts (7, `bolts`);
//! - `combat_arms:controller` (`ControllerParams`, `CaController`): a
//!   detonator; its round marks its owner's devices of one weapon (6.3,
//!   6.4, 7).
//!
//! Every object is a server entity (`CaObject`, the server's, authoritative:
//! a network client draws it from `NetItem`). Position and velocity are in
//! the data's world units, `CaTarget` is its box in metres for bullets
//! (`weapons`) and blasts (`grenades::blast_things`). On the server the
//! objects run on the grenades' 20 Hz frames (`grenades::CaServerFrames`,
//! `special_frames` after `grenades::server_frames`, in the spec's
//! per-tick order): each object in creation order (the removal tests, its
//! marks and triggers, its lifetime, then its physics: the first step in
//! the frame it was made in, its start time the frame before). Its
//! `Transform` (what the presentation and the network's `NetItem` show)
//! is drawn between those frames, one frame behind, as the grenades are
//! (`sync_objects`: from its place at the frame before to its place now),
//! so a client's snapshots, taken at its own update rate, see it move
//! every tick instead of 20 times a second (backlog, "Gun game playtest,
//! 2026-10-28"); its box (`CaTarget`) is where it is. Lasting
//! explosion spheres and over-time entries being `grenades`' (the
//! flames' and the napalm's spheres are `CaDamageSphere`s, the caltrops'
//! bleeding an `OverTime` entry keyed by the caltrops), the heals
//! (`CaHeals`) here. Blasts are `grenades::blast_bodies` and `blast_things`
//! (linear falloff from a quarter of the radius, cover traces, players
//! twice for the damage types that reach the player object and its hit
//! box, friendly fire by `core::DamageRules`, the armour step).
//!
//! Bullets and blasts take hit points off an object (`shot_objects`); at 1
//! or less it goes off where it is, credited to its owner (1.4). Devices
//! thrown by a player who leaves the game are removed silently; at the end
//! of a round (the match in round over, intermission or over) every device,
//! sphere and kit removes itself on its next update; impact projectiles in
//! flight go on (1.5).
//!
//! Known shortcuts (the spec's most literal reading where it is unclear,
//! listed for its open questions):
//! - No direct-hit damage from a projectile (every stock rocket and shell
//!   has `InstDamage` 0 and no type); no stun (`M32_Stun`'s `StunnedTime`)
//!   or freeze; the snowball gun (8) is not there; the heartbeat sensors,
//!   mission bomb, defence dummies and AI boss weapons (9) are other specs'.
//! - Projectiles don't touch each other (`CanHitSameKind`: two shells
//!   meeting); no water physics for them (Open question 10). Props stop a
//!   projectile at their box; a projectile's blast point on a prop is 1 u
//!   back along the flight.
//! - The fire key's time inside an animation isn't the weapon frame's:
//!   the frame fires the flamethrower, launchers and controllers when the
//!   press is processed (a controller's key at 25 ms, a launcher's at 0 are
//!   below a tick), devices leave the hand at their key by `Throwable`'s
//!   timing. Claymores are placed by the throw's launch (no `Velocity`).
//! - The turret's target is the nearest candidate that passes (the owner's
//!   client scan runs once per server frame here and is not the flicker of
//!   Quirk 3), its rays use the torso zone for any sphere hit and don't
//!   spend armour twice; nerve mine victims get `CaCameraReverse` (the
//!   effect itself is another spec's, Open question 8).
//! - A crossbow's alternate fire (right button, 1300 u/s, 41) isn't a mode
//!   yet; its bolt is the primary's. The explosive bolt's `Range` is
//!   measured in a straight line from the fire position.
//! - Placeholder looks: devices and bolts are shown by their weapon's
//!   `ShownItem` key (no models, effects or sounds for the devices yet);
//!   an impact projectile by its record's model (`ProjectileParams::
//!   shown_key`, from the install; a model-less one, the flame puff, and
//!   any without an install, by the front end's placeholder ball; the
//!   placeholder model of a model-less record with a scale, case 57, isn't
//!   drawn), facing its fire direction (`facing`, Follow-up 5); its look
//!   in flight (group, fly sound, flare, smoke trail: Follow-up 1-3) is
//!   carried by its drawn key and played by `effects::attached`. Its
//!   explosion's sound and group are the ImpactFX (`effects`). The projectile and device weapons have no view models
//!   of their own.
//! - Case 11 lists the M32's shells at 8100, 9100 ... and the end at 14134
//!   ms; section 3.2 and case 12 have each shell complete a second after
//!   the 2100 ms pre-reload and the reload end 1034 ms after the last
//!   (15134 ms from the 6000 ms start): followed. The claymore's cover
//!   traces run from its centre, not from the point 1 u off it. A stuck
//!   crossbow bolt sits 1 u off its wall, so its blast is not inside it.
//!   The hornet senses by the victim's centre within 100 u, not by his box.
//! - Rays and swings meet the first hit sphere on their line (a torso aim
//!   may meet an arm); the tests read the zone from the damage.
//! - Water, sky portals and invisible surfaces are `grenades`' (a clip
//!   brush is passed, sky removes the object).

pub mod bolts;
pub mod data;
pub mod devices;
pub mod impact;

use std::sync::Arc;

use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;

pub use self::data::*;
use super::{
    grenades::{
        CaGrenade, CaServerFrames, Frame, GrenadeWorld, Out, SERVER_FRAME,
    },
    rules::state::{CaMatch, Phase},
};
use crate::{
    core::{Damage, DamageKind, Died, Health, LocalPlayer, NetRole, Predict, SimClock, SimSet, Team},
    map::loose::ShownItem,
    net::{NetCharacter, NetItem, own_items::{OwnItemSettings, PredictedItem}},
    weapon::{
        CarriedAppExt, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponFrame, WeaponState,
        data::{WeaponData, WeaponDataAppExt},
        grenade::SurvivesRestart,
        random::shared_random,
    },
};

/// An object's box in metres, for bullets (`weapons`) and blasts
/// (`grenades::blast_things`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaTarget {
    pub centre: Vec3,
    pub half: Vec3,
}

/// What a special object is and the numbers it came with.
#[derive(Clone, Debug)]
pub enum Kind {
    Impact(Arc<ProjectileParams>),
    Device(Arc<DeviceParams>),
    Bolt(Arc<BoltParams>),
}

/// What changes in an object's life.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// A device on a floor (a placed claymore is landed at once), when
    /// (server time, s), and the time of its next floor re-snap (6.0).
    pub landed: bool,
    pub landed_at: f64,
    pub next_resnap: f64,
    /// Its facing (horizontal, unit): a claymore's cone, a kit's turn.
    pub facing: Vec3,
    /// The unit direction it moved in last (the blast point's 1 u back).
    pub last_dir: Vec3,
    /// The fire direction (unit): an impact projectile's model faces it
    /// for the whole flight (special_weapons.md Follow-up 5).
    pub launch_dir: Vec3,
    /// Set off at this time (mines, hornets): the blast follows the
    /// `active` animation.
    pub triggered_at: Option<f64>,
    /// A detonator pressed, or it was shot (claymores, remote grenades).
    pub marked: bool,
    /// A nerve mine touched a surface.
    pub active: bool,
    /// Gone on its next update with its effect (a replaced `OnceActivate`
    /// device).
    pub expire: bool,
    /// Caltrops: (victim, server time of his last count), the total.
    pub counted: Vec<(Entity, f64)>,
    pub counts: u32,
    /// The turret: rounds left, the next shot's time, the target, when it
    /// is set up.
    pub ammo: u32,
    pub next_shot: f64,
    pub target: Option<Entity>,
    pub ready_at: f64,
    /// The first-aid kit: the pool left and the next tick.
    pub pool: f32,
    pub next_tick: f64,
    /// The air strike: where it shoots from, the target point (u), when
    /// the strike began and the shells dropped so far.
    pub origin: Vec3,
    pub strike_point: Option<Vec3>,
    pub strike_at: Option<f64>,
    pub dropped: usize,
    /// A bolt: stuck in something (the entity, its offset), since when,
    /// the alternate fire's flag, and the explosive bolt's blast time.
    pub stuck: Option<(Entity, Vec3)>,
    pub stuck_at: f64,
    pub boom_at: Option<f64>,
}

/// A special weapon's object in the world (the server's).
#[derive(Component, Clone, Debug)]
pub struct CaObject {
    pub weapon: &'static str,
    pub owner: Option<Entity>,
    /// Metres per world unit, health per hit point.
    pub unit: f32,
    pub hp: f32,
    /// u, u/s.
    pub pos: Vec3,
    pub velocity: Vec3,
    /// Where it was and which way it was drawn facing at the server frame
    /// before (u, unit): it is drawn on the way from there to `pos` over
    /// the frame that follows (`sync_objects`), one server frame behind,
    /// as the grenades are (`grenades::draw_between_frames`).
    pub drawn_from: Vec3,
    pub drawn_dir_from: Vec3,
    /// Its box's half-extents, u.
    pub half: Vec3,
    /// Hit points left, and gone at 1 or less.
    pub health: f32,
    pub destroyed: bool,
    /// Creation order (updates run in it).
    pub seq: u64,
    /// The server frame its start time is (the one before its first).
    pub created: Option<u64>,
    /// Physics steps run.
    pub steps: u32,
    pub kind: Kind,
    pub s: State,
}

impl CaObject {
    pub fn new(weapon: &'static str, owner: Option<Entity>, unit: f32, hp: f32, pos: Vec3, velocity: Vec3, kind: Kind) -> Self {
        let (half, health) = match &kind {
            Kind::Impact(p) => (Vec3::from(p.box_half), p.health),
            Kind::Device(p) => (Vec3::from(p.box_half), 1.0),
            Kind::Bolt(p) => (Vec3::ZERO, p.health),
        };
        Self {
            weapon,
            owner,
            unit,
            hp,
            pos,
            velocity,
            drawn_from: pos,
            drawn_dir_from: Vec3::ZERO,
            half,
            health,
            destroyed: false,
            seq: 0,
            created: None,
            steps: 0,
            kind,
            s: State {
                facing: Vec3::NEG_Z,
                last_dir: velocity.normalize_or_zero(),
                launch_dir: velocity.normalize_or_zero(),
                ..default()
            },
        }
    }

    /// Its world position, metres.
    pub fn at(&self) -> Vec3 {
        self.pos * self.unit
    }

    /// Which way it is drawn facing (unit; zero: not turned): an impact
    /// projectile its fire direction, set once and not turned to follow a
    /// falling arc (special_weapons.md Follow-up 5); a bolt along its
    /// flight (a stuck bolt as it went in); else its facing.
    pub fn drawn_dir(&self) -> Vec3 {
        if matches!(self.kind, Kind::Impact(_)) && self.s.launch_dir != Vec3::ZERO {
            return self.s.launch_dir;
        }
        if !matches!(self.kind, Kind::Impact(_) | Kind::Bolt(_)) {
            return Vec3::ZERO;
        }
        if self.s.stuck.is_some() || self.velocity.length_squared() > 1.0 {
            self.velocity.normalize_or_zero()
        } else {
            self.s.facing
        }
    }

    /// Hit points taken by damage of `kind` (`weak`'s multipliers).
    fn weak(&self, kind: DamageKind) -> f32 {
        let ty = match kind {
            DamageKind::Bullet => 2,
            DamageKind::Blast => 7,
            DamageKind::Burn => 3,
            DamageKind::Melee => 13,
            _ => -1,
        };
        match &self.kind {
            Kind::Device(p) => p.weak.iter().find(|(t, _)| *t == ty).map_or(1.0, |(_, m)| *m),
            _ => 1.0,
        }
    }
}

/// The special frames' own count (the grenades' frame counter is shared:
/// `special_frames` runs the frames `grenades` has run) and the creation
/// counter.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CaSpecialFrames {
    pub frame: u64,
    pub next_seq: u64,
}

/// A heal over time (6.9): HP/s for the seconds left, at most `left`
/// hit points in all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heal {
    pub target: Entity,
    pub rate: f32,
    pub remaining: f32,
    pub left: f32,
    /// Health per hit point.
    pub hp: f32,
}

/// Heals to apply: the medkit's entries, and what the kits gave this frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct CaHeals {
    pub entries: Vec<Heal>,
    /// (target, health to add), applied (to the maximum) by `apply_heals`.
    pub now: Vec<(Entity, f32)>,
}

/// The nerve mine's effect on a victim (type 44, camera reverse): the
/// effect itself belongs to another spec (Open question 8); this marks
/// who has it and for how long.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaCameraReverse {
    pub amount: f32,
    pub until: f64,
}

// ---------------------------------------------------------------------------
// Weapon parts

/// Weapon part (`combat_arms:projectile`).
#[derive(Component, Clone, Debug)]
pub struct CaProjectileData {
    pub params: Arc<ProjectileParams>,
    pub unit: f32,
    pub hp: f32,
}

/// Weapon part (`combat_arms:device`).
#[derive(Component, Clone, Debug)]
pub struct CaDeviceData {
    pub params: Arc<DeviceParams>,
    pub unit: f32,
    pub hp: f32,
}

/// Weapon part (`combat_arms:bolt`).
#[derive(Component, Clone, Debug)]
pub struct CaBoltData {
    pub params: Arc<BoltParams>,
    pub unit: f32,
    pub hp: f32,
}

/// Weapon part (`combat_arms:controller`).
#[derive(Component, Clone, Debug)]
pub struct CaController {
    pub of: String,
}

/// Weapon part (`combat_arms:flame`): the fire keys' schedule (4).
#[derive(Component, Clone, Debug)]
pub struct CaFlame {
    pub params: FlameParams,
    pub state: FlameState,
}

/// Where the flamethrower's stream is.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FlameState {
    /// The stream's start (the press that began it), None when idle.
    pub since: Option<f64>,
    /// Keys fired since.
    pub keys: u32,
    /// The clip when last seen (a round gone is a key fired).
    pub clip: u32,
    /// No new stream before this (the post-fire).
    pub locked_until: f64,
}

impl NetPart for CaFlame {
    type Net = FlameState;
    fn net(&self) -> FlameState {
        self.state
    }
    fn set_net(&mut self, state: FlameState) {
        self.state = state;
    }
}

impl CaFlame {
    /// When fire key number `n` (from 0) of a stream that began at `since`
    /// goes: the pre-fire, then the loop's keys.
    pub fn key_time(&self, since: f64, n: u32) -> f64 {
        let p = &self.params;
        let keys = p.keys.max(1);
        since + p.prefire as f64 + (n / keys) as f64 * p.loop_len as f64 + (n % keys) as f64 * p.interval as f64
    }
}

// ---------------------------------------------------------------------------
// Plugin

pub struct CaSpecialPlugin;

impl Plugin for CaSpecialPlugin {
    fn build(&self, app: &mut App) {
        app.register_weapon_hook::<ProjectileParams>(PROJECTILE, |e, d: &'static WeaponData, p| {
            e.insert(CaProjectileData {
                params: Arc::new(p.clone()),
                unit: d.units.length,
                hp: d.units.damage,
            });
        })
        .register_weapon_hook::<FlameParams>(FLAME, |e, _, p| {
            e.insert(CaFlame {
                params: *p,
                state: FlameState::default(),
            });
        })
        .register_weapon_hook::<DeviceParams>(DEVICE, devices::device_weapon)
        .register_weapon_hook::<BoltParams>(BOLT, |e, d, p| {
            e.insert(CaBoltData {
                params: Arc::new(p.clone()),
                unit: d.units.length,
                hp: d.units.damage,
            });
        })
        .register_weapon_hook::<ControllerParams>(CONTROLLER, |e, _, p| {
            e.insert(CaController { of: p.of.clone() });
        })
        .init_resource::<CaSpecialFrames>()
        .init_resource::<CaHeals>()
        .add_message::<Damage>()
        .add_message::<Died>()
        .add_message::<WeaponEvent>()
        .add_observer(devices::on_launch)
        // The flame's keys pace the frame's trigger; predicted with it.
        .add_systems(Predict::Weapons, flame_pace.before(WeaponFrame))
        .predicted_part::<CaFlame>()
        .add_systems(
            FixedUpdate,
            (
                fire.after(WeaponFrame).before(special_frames),
                shot_objects.before(special_frames),
                special_frames.after(super::grenades::server_frames),
                (apply_heals, sync_objects).chain().after(special_frames),
            )
                .in_set(SimSet::Weapons)
                .run_if(crate::core::authoritative),
        )
        // A death marks the owner's devices; seen by the next frame.
        .add_systems(FixedUpdate, on_death.after(crate::core::apply_damage).run_if(crate::core::authoritative))
        // On a network server the clients draw the objects from `NetItem`s;
        // a client draws its own projectiles from the shot (`predict_shots`).
        .add_systems(FixedLast, net_items.run_if(resource_equals(NetRole::Server)))
        .add_systems(
            FixedUpdate,
            predict_shots
                .after(WeaponFrame)
                .in_set(SimSet::Weapons)
                .run_if(resource_equals(NetRole::Client)),
        );
    }
}

/// A network client's own impact projectiles and bolts, drawn from the
/// shot with `cl_predict_projectiles 1` (Chimera's `net::own_items`; the
/// original makes no client copy): the first time its predicted
/// weapon frame fires one, the same direction from the shared seed as
/// the server's (`fire`), at the record's speed, on the server's 20 Hz
/// steps (`SERVER_FRAME`) under the object gravity with its quarter term
/// (`grenades::flight_step`).
fn predict_shots(
    mut events: MessageReader<WeaponEvent>,
    local: Query<(), With<LocalPlayer>>,
    projectiles: Query<&CaProjectileData>,
    bolts: Query<(&CaBoltData, &Weapon)>,
    settings: Option<Res<OwnItemSettings>>,
    mut commands: Commands,
) {
    if !settings.is_some_and(|s| s.on()) {
        events.clear();
        return;
    }
    for e in events.read() {
        let WeaponEventKind::Fired {
            origin,
            yaw,
            pitch,
            seed,
            ..
        } = e.kind
        else {
            continue;
        };
        if e.replay || !local.contains(e.owner) {
            continue;
        }
        let step = super::grenades::SERVER_FRAME as f32;
        let predicted = if let Ok(d) = projectiles.get(e.weapon) {
            let p = &d.params;
            let dir = cone_dir(p.perturb, p.dynamic_perturb, yaw, pitch, seed, PROJECTILE);
            let gravity = if p.gravity { -super::grenades::OBJECT_GRAVITY * d.unit } else { 0.0 };
            PredictedItem::new(p.shown_key(), origin, dir * p.speed * d.unit, step)
                .with_gravity(gravity, super::grenades::GRAVITY_INTEGRATION_FACTOR)
        } else if let Ok((d, w)) = bolts.get(e.weapon) {
            let p = &d.params;
            let dir = cone_dir(p.perturb, p.dynamic_perturb, yaw, pitch, seed, BOLT);
            PredictedItem::new(w.id, origin, dir * p.speed * d.unit, step)
        } else {
            continue;
        };
        commands.spawn(predicted.with_shooter(e.owner).bundle());
    }
}

/// The hook name of a detonator.
pub const CONTROLLER: &str = "combat_arms:controller";

/// Pace the flamethrower's trigger by its fire keys (4): the stream
/// begins at the press, its first key after the pre-fire, then the keys
/// are `interval` apart in a loop of `loop_len`; each key fired moves the
/// next to its time. Letting go ends the stream; the post-fire holds the
/// next one.
fn flame_pace(
    owners: Query<(&crate::core::Intent, &crate::weapon::Inventory, Option<&Health>)>,
    mut weapons: Query<(&mut CaFlame, &mut WeaponState, Option<&crate::weapon::Magazine>)>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (intent, inv, health) in &owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((mut flame, mut st, mag)) = weapons.get_mut(w) else { continue };
            let clip = mag.map_or(0, |m| m.clip);
            let held = intent.fire && inv.active == Some(w) && alive && clip > 0;
            let mut s = flame.state;
            if clip < s.clip && s.since.is_some() {
                s.keys += s.clip - clip;
            }
            s.clip = clip;
            if !held {
                if s.since.is_some() {
                    s.since = None;
                    s.keys = 0;
                    s.locked_until = now + flame.params.postfire as f64;
                }
            } else {
                if s.since.is_none() {
                    s.since = Some(now.max(s.locked_until));
                    s.keys = 0;
                }
                let at = flame.key_time(s.since.unwrap_or(now), s.keys);
                if st.next_primary < at {
                    st.next_primary = at;
                }
                // The frame sets the next cycle after a round; the schedule
                // wins.
                if st.next_primary > at && s.keys > 0 {
                    st.next_primary = at;
                }
            }
            if flame.state != s {
                flame.state = s;
            }
        }
    }
}

/// A round of a projectile, bolt or controller weapon was fired: make the
/// object, or press the detonator (server).
fn fire(mut events: MessageReader<WeaponEvent>, weapons: Query<&Weapon>, mut commands: Commands) {
    for e in events.read() {
        if e.replay {
            continue;
        }
        let WeaponEventKind::Fired {
            origin,
            yaw,
            pitch,
            seed,
            ..
        } = e.kind
        else {
            continue;
        };
        if weapons.get(e.weapon).is_err() {
            continue;
        }
        let (weapon, owner) = (e.weapon, e.owner);
        commands.queue(move |w: &mut World| {
            if w.get::<CaProjectileData>(weapon).is_some() {
                impact::fire(w, weapon, owner, origin, yaw, pitch, seed);
            } else if w.get::<CaBoltData>(weapon).is_some() {
                bolts::fire(w, weapon, owner, origin, yaw, pitch, seed);
            } else if let Some(c) = w.get::<CaController>(weapon).cloned() {
                press(w, owner, &c.of);
            }
        });
    }
}

/// A detonator's press (6.3, 6.4, 7): the owner's claymores and remote
/// grenades of `of` are marked, his explosive bolts start their delay.
pub fn press(world: &mut World, owner: Entity, of: &str) {
    let frame_time = {
        let f = world.resource::<CaServerFrames>();
        f.time()
    };
    let mut q = world.query::<&mut CaObject>();
    for mut o in q.iter_mut(world) {
        if o.owner != Some(owner) || o.weapon != of {
            continue;
        }
        match &o.kind {
            Kind::Device(_) => o.s.marked = true,
            Kind::Bolt(p) => {
                if let Some(x) = &p.explosive
                    && o.s.boom_at.is_none()
                {
                    let delay = x.delay as f64;
                    o.s.boom_at = Some(frame_time + delay);
                }
            }
            Kind::Impact(_) => {}
        }
    }
}

/// An owner's death detonates his claymores and remote grenades (6.3) and
/// his explosive bolts (7).
fn on_death(mut died: MessageReader<Died>, mut objects: Query<&mut CaObject>) {
    for d in died.read() {
        for mut o in &mut objects {
            if o.owner != Some(d.entity) {
                continue;
            }
            match &o.kind {
                Kind::Device(p) if matches!(p.kind, DeviceKind::Claymore | DeviceKind::RemoteGrenade) => o.s.marked = true,
                Kind::Bolt(p) if p.explosive.is_some() => o.destroyed = true,
                _ => {}
            }
        }
    }
}

/// Damage to an object (bullets, blasts): off its hit points by its weak
/// points; at 1 or less it goes off at its next update (1.4). A device
/// still in the air can't be set off (Quirk 7).
fn shot_objects(mut damage: MessageReader<Damage>, mut objects: Query<&mut CaObject>) {
    for d in damage.read() {
        let Ok(mut o) = objects.get_mut(d.target) else { continue };
        if d.amount <= 0.0 || o.destroyed {
            continue;
        }
        if matches!(o.kind, Kind::Device(_)) && !o.s.landed {
            continue;
        }
        let taken = d.amount / o.hp * o.weak(d.kind);
        o.health -= taken;
        if o.health <= super::grenades::DESTROYED_AT {
            o.destroyed = true;
        }
    }
}

/// Heals reach health (the maximum caps it).
pub(super) fn apply_heals(mut heals: ResMut<CaHeals>, mut health: Query<&mut Health>) {
    for (target, amount) in std::mem::take(&mut heals.now) {
        if let Ok(mut h) = health.get_mut(target)
            && h.current > 0.0
        {
            h.current = (h.current + amount).min(h.max);
        }
    }
}

/// Objects in flight or lying about, for the clients to draw: Chimera's
/// `NetItem` (the weapon's `ShownItem` key and the pose), replicated from
/// the first one on.
fn net_items(
    mut q: Query<(Entity, &Transform, &ShownItem, &CaObject, Option<&mut NetItem>)>,
    characters: Query<&NetCharacter>,
    mut commands: Commands,
) {
    for (e, t, item, o, have) in &mut q {
        let now = NetItem {
            model: item.0.clone(),
            origin: t.translation.to_array(),
            rotation: t.rotation.to_array(),
            owner: o.owner.and_then(|w| characters.get(w).ok()).and_then(|c| c.owner),
        }
        .quantized();
        match have {
            Some(mut have) => {
                have.set_if_neq(now);
            }
            None => {
                commands.entity(e).insert((Replicated, now));
            }
        }
    }
}

/// Objects' boxes follow their positions; their transforms (what the
/// presentation and the network's `NetItem` show) follow them between
/// server frames: on the way from the frame before (`drawn_from`) to now,
/// by how far the clock is into the next frame (`CaServerFrames::blend`).
/// Objects move only on the server's 20 frames a second: drawn at them
/// they hopped 50 ms at a time, and a client's snapshots (taken at its
/// update rate, held at the ticks between) drew a staircase of stops and
/// jumps (backlog, "Gun game playtest, 2026-10-28").
pub(super) fn sync_objects(
    frames: Res<CaServerFrames>,
    clock: Res<SimClock>,
    mut objects: Query<(&CaObject, &mut Transform, &mut CaTarget)>,
) {
    let a = frames.blend(clock.now);
    for (o, mut t, mut target) in &mut objects {
        let at = o.drawn_from.lerp(o.pos, a) * o.unit;
        if t.translation != at {
            t.translation = at;
        }
        let to = o.drawn_dir();
        let dir = if o.drawn_dir_from == Vec3::ZERO {
            to
        } else {
            o.drawn_dir_from.lerp(to, a).normalize_or(to)
        };
        if dir != Vec3::ZERO {
            let r = facing(dir);
            if t.rotation != r {
                t.rotation = r;
            }
        }
        let tg = CaTarget {
            centre: o.at(),
            half: o.half * o.unit,
        };
        if *target != tg {
            *target = tg;
        }
    }
}

/// The rotation that faces `dir` (unit) with the world's up made square to
/// it, or for a shot straight up or down, world +X (special_weapons.md
/// Follow-up 5).
pub fn facing(dir: Vec3) -> Quat {
    let up = if dir.cross(Vec3::Y).length_squared() < 1e-8 { Vec3::X } else { Vec3::Y };
    Transform::IDENTITY.looking_to(dir, up).rotation
}

// ---------------------------------------------------------------------------
// Making objects

/// Add `o` to the world: creation order, its transform, box and name.
pub fn spawn_object(world: &mut World, mut o: CaObject) -> Entity {
    let seq = {
        let mut f = world.resource_mut::<CaSpecialFrames>();
        f.next_seq += 1;
        f.next_seq
    };
    o.seq = seq;
    let at = o.at();
    let name = format!("{} {}", o.weapon, seq);
    let survives = matches!(o.kind, Kind::Impact(_));
    let target = CaTarget {
        centre: at,
        half: o.half * o.unit,
    };
    // Drawn as its projectile record's model (impact projectiles), else
    // by its weapon's key.
    let id = match &o.kind {
        Kind::Impact(p) => p.shown_key(),
        _ => o.weapon.to_string(),
    };
    // A device's sounds in flight and at rest (Corrections (2026-10-31) 3).
    let look = match &o.kind {
        Kind::Device(d) if d.look.has_sounds() => Some(d.look.clone()),
        Kind::Bolt(b) if b.look.has_sounds() => Some(b.look.clone()),
        _ => None,
    };
    let mut e = world.spawn((Name::new(name), Transform::from_translation(at), target, ShownItem(id), o));
    if survives {
        e.insert(SurvivesRestart);
    }
    if let Some(look) = look {
        e.insert((
            super::effects::attached::CaFlightSounds::default(),
            super::effects::attached::CaFlightLook(look),
        ));
    }
    e.id()
}

// ---------------------------------------------------------------------------
// The server's frames

/// What the frames need besides the world and the damage output.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Extra<'w, 's> {
    pub teams: Query<'w, 's, &'static Team>,
    pub grenades: Query<'w, 's, &'static mut CaGrenade>,
    pub heals: ResMut<'w, CaHeals>,
    pub round: Option<Res<'w, CaMatch>>,
}

impl Extra<'_, '_> {
    /// Whether `a` and `b` are on one team (free-for-all players have a
    /// team each; none: no team).
    pub fn same_team(&self, a: Entity, b: Entity) -> bool {
        match (self.teams.get(a), self.teams.get(b)) {
            (Ok(x), Ok(y)) => x == y && x.0 != 0,
            _ => false,
        }
    }

    /// `other` is an enemy of `owner`: not him, not on his team.
    pub fn enemy(&self, owner: Option<Entity>, other: Entity) -> bool {
        match owner {
            Some(o) => o != other && !self.same_team(o, other),
            None => true,
        }
    }

    /// The round is over: devices go (1.5).
    pub fn round_over(&self) -> bool {
        self.round
            .as_ref()
            .is_some_and(|m| matches!(m.phase, Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver))
    }
}

/// One frame's working set.
pub(crate) struct Cx<'a, 'w, 's, 'ow, 'os, 'xw, 'xs> {
    pub f: &'a mut Frame,
    pub world: &'a GrenadeWorld<'w, 's>,
    pub out: &'a mut Out<'ow, 'os>,
    pub x: &'a mut Extra<'xw, 'xs>,
    pub round_over: bool,
}

impl Cx<'_, '_, '_, '_, '_, '_, '_> {
    /// Take object `e` out of the world.
    pub fn remove(&mut self, e: Entity) {
        self.f.gone.push(e);
        self.out.commands.entity(e).try_despawn();
    }

    /// Make a child object (a shell of a strike) from the commands.
    pub fn spawn(&mut self, o: CaObject) {
        self.out.commands.queue(move |w: &mut World| {
            spawn_object(w, o);
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn special_frames(
    grenade_frames: Res<CaServerFrames>,
    clock: Res<SimClock>,
    mut mine: ResMut<CaSpecialFrames>,
    mut objects: Query<(Entity, &mut CaObject)>,
    world: GrenadeWorld,
    mut out: Out,
    mut x: Extra,
    spheres: Query<Entity, With<super::grenades::CaDamageSphere>>,
) {
    // The lasting spheres go with the round (1.5, Open question 1).
    if x.round_over() {
        for e in &spheres {
            out.commands.entity(e).try_despawn();
        }
    }
    while mine.frame < grenade_frames.frame {
        mine.frame += 1;
        let mut f = Frame::new(
            mine.frame,
            mine.frame as f64 * grenade_frames.interval,
            clock.now,
            grenade_frames.interval as f32,
        );
        let mut order: Vec<(u64, Entity)> = objects.iter().map(|(e, o)| (o.seq, e)).collect();
        order.sort_unstable_by_key(|(s, _)| *s);
        let round_over = x.round_over();
        for (_, e) in order {
            if f.gone.contains(&e) {
                continue;
            }
            let Ok((_, mut o)) = objects.get_mut(e) else { continue };
            let mut cx = Cx {
                f: &mut f,
                world: &world,
                out: &mut out,
                x: &mut x,
                round_over,
            };
            o.created.get_or_insert(mine.frame - 1);
            o.drawn_from = o.pos;
            o.drawn_dir_from = o.drawn_dir();
            match o.kind.clone() {
                Kind::Impact(p) => impact::update(&mut cx, e, &mut o, &p),
                Kind::Device(p) => devices::update(&mut cx, e, &mut o, &p),
                Kind::Bolt(p) => bolts::update(&mut cx, e, &mut o, &p),
            }
        }
        // 4. Heals (6.9): each entry gives its rate for the frame.
        let dt = f.dt;
        let entries = std::mem::take(&mut x.heals.entries);
        for mut h in entries {
            let give = (h.rate * dt).min(h.left).max(0.0);
            x.heals.now.push((h.target, give * h.hp));
            h.left -= give;
            h.remaining -= dt;
            if h.remaining > 1e-4 && h.left > 1e-6 {
                x.heals.entries.push(h);
            }
        }
    }
}

/// Seconds since object `o` was created, in whole server frames.
pub(crate) fn age(o: &CaObject, f: &Frame) -> f64 {
    (f.frame - o.created.unwrap_or(f.frame - 1)) as f64 * f.dt as f64
}

/// A server frame, s.
pub const FRAME: f64 = SERVER_FRAME;

/// The aim of a view: yaw and pitch as a rotation.
pub(crate) fn aim(yaw: f32, pitch: f32) -> Quat {
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// The cone's direction for a round (the meters at 0): a uniform square
/// of n thousandths about the aim.
pub(crate) fn cone_dir(perturb: [u32; 2], dynamic: f32, yaw: f32, pitch: f32, seed: u32, label: &str) -> Vec3 {
    let n = super::grenades::throw_perturb(perturb, dynamic, 0.0);
    let mut rng = shared_random(seed, label, 0);
    let (u1, u2) = (rng.next(), rng.next());
    super::weapons::square_dir(aim(yaw, pitch), n / 1000.0, u1, u2)
}
