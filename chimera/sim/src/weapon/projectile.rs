//! Launched projectiles for any game (lift C1, docs/plans/active/hl2dm.md
//! section 3): a flying body (`Flyer`) with its speed, gravity, box (or
//! sphere, or point), a lifetime, a grace while it passes through
//! everything, and what touching something does (`Touch`): explode (a
//! `grenade::BlastAt` at the point), bounce off everything (keeping its
//! speed, `Flyer::keep_speed`), stick or glance off like a bolt, or only
//! report it (a bolt sticks in what doesn't move: static bodies, and
//! kinematic ones at rest, the logic's brush entities by their
//! `MovingSolid`); a direct hit's damage (`Direct`: through the shared armour
//! step, `HitArmor`; the head's multiplier when it hits characters'
//! hitboxes). Games steer flyers (guidance, seeking) in `FlyerSet::Think`
//! and answer `FlyerHit`s in `FlyerSet::Touch`. Movement is Source's toss
//! (`grenade::gravity_step`, exact for constant gravity) and a swept shape
//! against `core::SOLID_LAYERS`, never touching its owner (nor loose
//! items with `Flyer::pass_items`, nor anything before
//! `Flyer::solid_after`); a flyer made during a tick first moves on the
//! next. The server's (`core::authoritative`); clients draw it from the
//! network's `NetItem` (its `ShownItem` and `Transform`, facing its
//! flight), as thrown grenades.
//!
//! Made by a game's `LaunchRequest` (an alt-fire, a rocket) or from a
//! weapon's frame rounds: a weapon with `chimera:projectile`
//! (`LauncherParams`, in the data's units; `Launcher`) turns each of its
//! `Fired` rounds into a flyer from the eye along the shot's angles
//! (`Fired` without a `DamageEffect`: the frame traces nothing), at
//! `water_speed` when its owner is marked `LaunchUnderWater`.
//!
//! Also here, shared with Combat Arms' special weapons (whose objects run
//! on their own 20 Hz frames): the ray tests `ray_box` and `ray_sphere`,
//! and `facing` (the rotation a model flying along a direction is drawn
//! with).
//!
//! Units: meters, seconds, damage normalized like `Health`.
//!
//! A flyer with `Flyer::sky_end` ends on the sky (a line that far along its
//! flight meets `map::MapSkyFaces` with nothing before it): gone, no touch
//! rule, its `FlyerGone::expired` as said.
//!
//! Known shortcuts: a bounce or a glance spends the rest of its tick's move; the
//! hitbox test of `Flyer::hitboxes` runs on the swept line's segment
//! through the character's hull (a hit on the hull that misses every
//! hitbox lets it fly on); breakable glass stops a bolt like any body; a
//! brush entity turning in place (a rotating door: its `MovingSolid` has
//! no angular speed) counts as at rest, and a bolt stuck in a brush stays
//! where it stuck when the brush moves later;
//! nothing is predicted (the spec's own: HL2:DM's projectiles appear one
//! round trip late).

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;

use super::{
    HitArmor, Inflicted, Weapon, WeaponEvent, WeaponEventKind,
    data::{WeaponData, WeaponDataAppExt},
    grenade::{Blast, BlastAt, gravity_step},
};
use crate::{
    core::{Damage, DamageKind, Damageable, Health, Hitgroup, ITEM_LAYER, Intent, MovingSolid, SOLID_LAYERS, SimClock},
    map::loose::ShownItem,
};

/// The weapon hook of a weapon whose rounds are flyers (`LauncherParams`).
pub const LAUNCHER: &str = "chimera:projectile";

/// Gap kept between a flyer and what it touched, m.
const SKIN: f32 = 1e-3;

/// The flyers' steps within `SimSet::Weapons`, after the weapon frame:
/// new flyers from launches, games' steering, the moves, games' answers
/// to the touches (and the blasts asked for, after them).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FlyerSet {
    Launch,
    Think,
    Move,
    Touch,
}

/// What touching something does (the flyer stops where it touched first).
#[derive(Clone, Debug, PartialEq)]
pub enum Touch {
    /// Report it (`FlyerHit`) and stop; the game decides.
    Report,
    /// Its blast (`Flyer::blast`) at the touch point; gone.
    Explode,
    /// Bounce off (mirrored, at `Flyer::keep_speed` if set, else the
    /// same speed); the direct damage to what takes it.
    Bounce,
    /// A bolt: its direct damage to what takes damage, then gone; off a
    /// non-moving surface it glances (`BoltRule`) or sticks.
    Bolt(BoltRule),
}

/// A bolt's touch on a surface (specs/hl2dm/projectiles.md 7.4): with
/// `c` = n·(−v̂), below `glance_cos` and faster than `glance_speed` (m/s)
/// it reflects at `glance_keep` of its speed with `glance_gravity`
/// (m/s²) from then on; else it sticks, still, and is gone `stick_time`
/// later.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoltRule {
    pub glance_cos: f32,
    pub glance_speed: f32,
    pub glance_keep: f32,
    pub glance_gravity: f32,
    pub stick_time: f32,
}

/// A direct hit's damage on what it touches that takes damage:
/// `damage` (normalized) of `kind`, the head's multiplier `head` on a
/// hitbox hit there, the armour ratio and quantum as a gun's, a push of
/// `force` (kg·m/s) along its flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direct {
    pub damage: f32,
    pub kind: DamageKind,
    pub head: f32,
    pub armor_ratio: Option<f32>,
    pub quantum: f32,
    pub force: f32,
}

/// A projectile in flight (the server's).
#[derive(Component, Clone, Debug)]
pub struct Flyer {
    pub owner: Option<Entity>,
    /// The weapon ID it came from (kill notices, drawing).
    pub weapon: &'static str,
    /// m/s.
    pub velocity: Vec3,
    /// Downward acceleration, m/s².
    pub gravity: f32,
    /// Its box's half-extents, m (zero: a point); with `sphere` a sphere
    /// of radius `half.x`.
    pub half: Vec3,
    pub sphere: bool,
    /// When it was made (game seconds) and on which tick (it moves from
    /// the next).
    pub born: f64,
    pub born_tick: u64,
    /// Moves made.
    pub ticks: u32,
    /// It passes through everything before this time.
    pub solid_after: f64,
    /// Gone at this time (`FlyerGone::expired`).
    pub expires: Option<f64>,
    /// Its speed after every contact (m/s).
    pub keep_speed: Option<f32>,
    /// It passes loose items (dropped weapons, `core::ITEM_LAYER`).
    pub pass_items: bool,
    /// Characters with hitboxes are hit only where a hitbox is.
    pub hitboxes: bool,
    pub touch: Touch,
    pub direct: Option<Direct>,
    pub blast: Option<Arc<Blast>>,
    /// A stuck bolt, gone at this time.
    pub stuck_until: Option<f64>,
    /// It ends touching the sky.
    pub sky_end: Option<SkyEnd>,
    /// Hit this tick: what (the game's `FlyerSet::Touch` reads `FlyerHit`).
    pub last_hit: Option<Entity>,
    /// Its model's own turn before it faces its flight (`facing` puts −Z
    /// along it; a Source model's nose is its +X: `MODEL_X_FORWARD`).
    pub turn: Quat,
}

/// How a flyer ends on the sky: the line ahead that finds it (m), and
/// whether `FlyerGone` says its time ran out (a ball's harmless end) or not
/// (silently gone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyEnd {
    pub probe: f32,
    pub expired: bool,
}

/// `Flyer::turn` for a model whose nose is its +X (Source's models).
pub const MODEL_X_FORWARD: Quat = Quat::from_xyzw(0.0, std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2);

/// `Flyer::turn` for a model whose nose is its +Y (a quarter turn about X
/// takes +Y to −Z).
pub const MODEL_Y_FORWARD: Quat = Quat::from_xyzw(-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2);

impl Flyer {
    /// A flyer of `weapon` by `owner` with `velocity` (m/s), made now: a
    /// point with no gravity that reports its touches; set the rest.
    pub fn new(weapon: &'static str, owner: Option<Entity>, velocity: Vec3, clock: &SimClock) -> Self {
        Self {
            owner,
            weapon,
            velocity,
            gravity: 0.0,
            half: Vec3::ZERO,
            sphere: false,
            born: clock.now,
            born_tick: clock.tick,
            ticks: 0,
            solid_after: f64::MIN,
            expires: None,
            keep_speed: None,
            pass_items: false,
            hitboxes: false,
            touch: Touch::Report,
            direct: None,
            blast: None,
            stuck_until: None,
            sky_end: None,
            last_hit: None,
            turn: Quat::IDENTITY,
        }
    }
}

/// A flyer touched something (after its move, before games answer in
/// `FlyerSet::Touch`): what (`other`: the body; None: the world), where,
/// the surface normal there, its velocity before the touch, whether it is
/// a character, and whether the flyer is gone by it (exploded, a bolt
/// spent).
#[derive(Message, Clone, Copy, Debug)]
pub struct FlyerHit {
    pub flyer: Entity,
    pub weapon: &'static str,
    pub owner: Option<Entity>,
    pub other: Option<Entity>,
    pub point: Vec3,
    pub normal: Vec3,
    pub velocity: Vec3,
    pub character: bool,
    /// What it touched doesn't move (the world, static brushes).
    pub world: bool,
    pub gone: bool,
}

/// A flyer is gone: its lifetime ran out (`expired`), or it touched
/// something and exploded or was spent. Where it was.
#[derive(Message, Clone, Copy, Debug)]
pub struct FlyerGone {
    pub flyer: Entity,
    pub weapon: &'static str,
    pub owner: Option<Entity>,
    pub at: Vec3,
    pub expired: bool,
}

/// A game's request for a flyer (an alt-fire's grenade, a rocket): made
/// in `FlyerSet::Launch` at `start` with its `velocity` set, `flyer`
/// otherwise as given (its clock fields are set then), shown as `shown`.
/// Its `Entity` is told back on `Launched`.
#[derive(Message, Clone, Debug)]
pub struct LaunchRequest {
    pub flyer: Flyer,
    pub start: Vec3,
    pub shown: Option<String>,
    /// What the game wants on the new entity besides (its own parts).
    pub tag: Option<u64>,
}

/// A flyer made from a `LaunchRequest` (its `tag`), with its velocity
/// and tick: the entity is spawned at the next sync point (a game's
/// commands inserting its own parts on it apply after the spawn).
#[derive(Message, Clone, Copy, Debug)]
pub struct Launched {
    pub flyer: Entity,
    pub owner: Option<Entity>,
    pub weapon: &'static str,
    pub tag: Option<u64>,
    pub velocity: Vec3,
    pub tick: u64,
}

/// A game's projectile that moves by its own rules (a physics body: HL2's
/// frags and satchels; a placed mine) and is drawn on clients as flyers
/// are (its `ShownItem`, the network's `NetItem`), with its owner.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShownBody {
    pub owner: Option<Entity>,
}

/// On a character whose launches go at their `water_speed`
/// (`LauncherParams`): its eyes are under water (the game's movement
/// says).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LaunchUnderWater;

/// `chimera:projectile`'s parameters, in the weapon data's units
/// (lengths by `units.length`, damage by `units.damage`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LauncherParams {
    pub speed: f32,
    /// Its speed when the shooter's eyes are under water.
    #[serde(default)]
    pub water_speed: Option<f32>,
    #[serde(default)]
    pub gravity: f32,
    /// Its box's half-extents (a point: none).
    #[serde(default)]
    pub half: Option<[f32; 3]>,
    /// Its model key (`ShownItem`); none: the weapon's.
    #[serde(default)]
    pub shown: Option<String>,
    /// "report", "explode", "bounce" or "bolt".
    #[serde(default = "report")]
    pub touch: String,
    /// A bolt's glance and stick (`BoltRule`; speeds in the data's units).
    #[serde(default)]
    pub glance_cos: f32,
    #[serde(default)]
    pub glance_speed: f32,
    #[serde(default)]
    pub glance_keep: f32,
    #[serde(default)]
    pub glance_gravity: f32,
    #[serde(default)]
    pub stick_time: f32,
    /// Direct damage (none: 0) of `kind` ("bullet", "crush", "blast",
    /// "melee", "burn"; default bullet), the head's multiplier, the push
    /// (kg × length unit / s), the armour ratio.
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default = "one")]
    pub head: f32,
    #[serde(default)]
    pub force: f32,
    #[serde(default)]
    pub armor_ratio: Option<f32>,
    /// Characters with hitboxes are hit only there.
    #[serde(default)]
    pub hitboxes: bool,
    #[serde(default)]
    pub pass_items: bool,
    /// Lifetime, seconds.
    #[serde(default)]
    pub life: Option<f32>,
    /// Its model's nose is its +X (Source's models: `MODEL_X_FORWARD`).
    #[serde(default)]
    pub x_forward: bool,
    /// Its model's nose is its +Y (the crossbow bolt's, as the loose frame
    /// leaves it: `MODEL_Y_FORWARD`).
    #[serde(default)]
    pub y_forward: bool,
    /// Ends on the sky a line this long finds (data length; 0: never).
    #[serde(default)]
    pub sky_probe: f32,
}

fn report() -> String {
    "report".into()
}

fn one() -> f32 {
    1.0
}

/// A damage kind by its data name.
pub fn damage_kind(name: Option<&str>) -> DamageKind {
    match name.unwrap_or("bullet") {
        "crush" => DamageKind::Crush,
        "blast" => DamageKind::Blast,
        "melee" => DamageKind::Melee,
        "burn" => DamageKind::Burn,
        "generic" => DamageKind::Generic,
        _ => DamageKind::Bullet,
    }
}

/// Weapon part (`chimera:projectile`): its rounds are flyers like this
/// (meters; `Flyer::velocity` the unit direction's speed is `speed`).
#[derive(Component, Clone, Debug)]
pub struct Launcher {
    pub speed: f32,
    pub water_speed: Option<f32>,
    pub shown: String,
    pub template: Flyer,
}

impl LauncherParams {
    /// The flyer these make, in meters (`unit` m per data length,
    /// `damage` health per data damage point).
    pub fn launcher(&self, weapon: &'static str, unit: f32, damage: f32) -> Launcher {
        let clock = SimClock::default();
        let mut f = Flyer::new(weapon, None, Vec3::ZERO, &clock);
        f.gravity = self.gravity * unit;
        f.half = self
            .half
            .map_or(Vec3::ZERO, |h| Vec3::new(h[0], h[2], h[1]).abs() * unit);
        f.pass_items = self.pass_items;
        f.sky_end = (self.sky_probe > 0.0).then_some(SkyEnd {
            probe: self.sky_probe * unit,
            expired: false,
        });
        f.hitboxes = self.hitboxes;
        if self.x_forward {
            f.turn = MODEL_X_FORWARD;
        } else if self.y_forward {
            f.turn = MODEL_Y_FORWARD;
        }
        // Relative here; made absolute at the launch.
        f.expires = self.life.map(|l| l as f64);
        f.touch = match self.touch.as_str() {
            "explode" => Touch::Explode,
            "bounce" => Touch::Bounce,
            "bolt" => Touch::Bolt(BoltRule {
                glance_cos: self.glance_cos,
                glance_speed: self.glance_speed * unit,
                glance_keep: self.glance_keep,
                glance_gravity: self.glance_gravity * unit,
                stick_time: self.stick_time,
            }),
            _ => Touch::Report,
        };
        f.direct = (self.damage > 0.0).then(|| Direct {
            damage: self.damage * damage,
            kind: damage_kind(self.kind.as_deref()),
            head: self.head,
            armor_ratio: self.armor_ratio,
            quantum: 0.0,
            force: self.force * unit,
        });
        Launcher {
            speed: self.speed * unit,
            water_speed: self.water_speed.map(|s| s * unit),
            shown: self.shown.clone().unwrap_or_else(|| weapon.to_string()),
            template: f,
        }
    }
}

/// Register `chimera:projectile` (the weapon plugin does; a bare app
/// loading weapon data that names it, too).
pub fn register_launcher(app: &mut App) -> &mut App {
    app.register_weapon_hook::<LauncherParams>(LAUNCHER, |e, d: &'static WeaponData, p| {
        let id = e.get::<Weapon>().map_or("", |w| w.id);
        e.insert(p.launcher(id, d.units.length, d.units.damage));
    })
}

pub(super) fn plugin(app: &mut App) {
    register_launcher(app)
        .add_message::<FlyerHit>()
        .add_message::<FlyerGone>()
        .add_message::<LaunchRequest>()
        .add_message::<Launched>()
        .add_message::<BlastAt>()
        .add_message::<Inflicted>()
        .configure_sets(
            FixedUpdate,
            // No sync points of their own (the schedule ratchets): a flyer
            // made in `Launch` is seen from the next tick (it first moves
            // then anyway), one gone in `Move` is gone by the next.
            (FlyerSet::Launch, FlyerSet::Think, FlyerSet::Move, FlyerSet::Touch)
                .chain_ignore_deferred()
                .after(super::WeaponFrame)
                .in_set(crate::core::SimSet::Weapons),
        )
        .add_systems(
            FixedUpdate,
            (
                (launch_from_fired, launch_requests).chain_ignore_deferred().in_set(FlyerSet::Launch),
                (expire, fly).chain_ignore_deferred().in_set(FlyerSet::Move),
            )
                // They move only flyers and write messages read after the
                // weapons (damage, blasts, touches): unordered against
                // other games' systems on purpose (the schedule ratchets).
                .ambiguous_with_all()
                .run_if(crate::core::authoritative),
        );
}

// ---------------------------------------------------------------------------
// Shared geometry (lifted from Combat Arms' special weapons)

/// Where a ray from `from` along unit `dir` enters the box `min`..`max`
/// within `limit` (0 from inside), if it does.
pub fn ray_box(min: Vec3, max: Vec3, from: Vec3, dir: Vec3, limit: f32) -> Option<f32> {
    let (mut enter, mut leave) = (f32::MIN, f32::MAX);
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if from[i] < min[i] || from[i] > max[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((min[i] - from[i]) / dir[i], (max[i] - from[i]) / dir[i]);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter <= leave && leave >= 0.0 && enter.max(0.0) <= limit).then_some(enter.max(0.0))
}

/// Where the segment `from`..`to` enters the sphere (`centre`, `r`), as a
/// fraction of it (0 from inside), if it does.
pub fn ray_sphere(from: Vec3, to: Vec3, centre: Vec3, r: f32) -> Option<f32> {
    let d = to - from;
    let a = d.length_squared();
    if a <= 0.0 || r <= 0.0 {
        return None;
    }
    let m = from - centre;
    let c = m.length_squared() - r * r;
    if c <= 0.0 {
        return Some(0.0);
    }
    let b = m.dot(d);
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// The rotation of a model flying along `dir` (its −Z forward, up kept
/// up unless it flies straight up or down).
pub fn facing(dir: Vec3) -> Quat {
    let up = if dir.cross(Vec3::Y).length_squared() < 1e-8 {
        Vec3::X
    } else {
        Vec3::Y
    };
    Transform::IDENTITY.looking_to(dir, up).rotation
}

/// The reflection of `v` on a surface of normal `n`.
pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - 2.0 * v.dot(n) * n
}

// ---------------------------------------------------------------------------
// Making flyers

/// Spawn `flyer` at `start`, drawn as `shown`, made now.
pub fn spawn_flyer(commands: &mut Commands, mut flyer: Flyer, start: Vec3, shown: String, clock: &SimClock) -> Entity {
    flyer.born = clock.now;
    flyer.born_tick = clock.tick;
    if flyer.solid_after == f64::MIN {
        flyer.solid_after = clock.now;
    }
    let rotation = facing(flyer.velocity.normalize_or(Vec3::NEG_Z)) * flyer.turn;
    commands
        .spawn((
            Name::new(format!("Projectile {}", flyer.weapon)),
            Transform::from_translation(start).with_rotation(rotation),
            flyer,
            ShownItem(shown),
        ))
        .id()
}

/// Each `Fired` round of a `Launcher` weapon (the server's, not a replay):
/// a flyer from the eye along the round's angles.
fn launch_from_fired(
    mut events: MessageReader<WeaponEvent>,
    launchers: Query<&Launcher>,
    under: Query<(), With<LaunchUnderWater>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, .. } = e.kind else {
            continue;
        };
        if e.replay {
            continue;
        }
        let Ok(l) = launchers.get(e.weapon) else { continue };
        let speed = match l.water_speed {
            Some(s) if under.contains(e.owner) => s,
            _ => l.speed,
        };
        let dir = super::aim_of(yaw, pitch) * Vec3::NEG_Z;
        let mut f = l.template.clone();
        f.owner = Some(e.owner);
        f.velocity = dir * speed;
        if let Some(life) = f.expires {
            f.expires = Some(clock.now + life);
        }
        spawn_flyer(&mut commands, f, origin, l.shown.clone(), &clock);
    }
}

/// Games' `LaunchRequest`s.
fn launch_requests(
    mut requests: MessageReader<LaunchRequest>,
    mut launched: MessageWriter<Launched>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let shown = r.shown.clone().unwrap_or_else(|| r.flyer.weapon.to_string());
        let e = spawn_flyer(&mut commands, r.flyer.clone(), r.start, shown, &clock);
        launched.write(Launched {
            flyer: e,
            owner: r.flyer.owner,
            weapon: r.flyer.weapon,
            tag: r.tag,
            velocity: r.flyer.velocity,
            tick: clock.tick,
        });
    }
}

// ---------------------------------------------------------------------------
// Flight

#[derive(bevy::ecs::query::QueryData)]
struct Body {
    entity: Entity,
    transform: &'static Transform,
    aabb: Option<&'static ColliderAabb>,
    health: Option<&'static Health>,
    damageable: Option<&'static Damageable>,
    intent: Option<&'static Intent>,
    hitboxes: crate::core::HitShapes,
}

/// The world access flyers need.
#[derive(bevy::ecs::system::SystemParam)]
struct FlyWorld<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    sky: Option<Res<'w, crate::map::MapSkyFaces>>,
    bodies: Query<'w, 's, Body, Without<Flyer>>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    rigid: Query<'w, 's, (&'static RigidBody, Option<&'static MovingSolid>, Option<&'static LinearVelocity>, Option<&'static AngularVelocity>)>,
    flyers: Query<'w, 's, (), With<Flyer>>,
    armor: HitArmor<'w, 's>,
    damage: MessageWriter<'w, Damage>,
    inflicted: MessageWriter<'w, Inflicted>,
    hits: MessageWriter<'w, FlyerHit>,
    gone: MessageWriter<'w, FlyerGone>,
    blasts: MessageWriter<'w, BlastAt>,
}

impl FlyWorld<'_, '_> {
    fn body(&self, collider: Entity) -> Entity {
        self.colliders.get(collider).map_or(collider, |c| c.body)
    }

    fn is_character(&self, e: Entity) -> bool {
        self.bodies.get(e).is_ok_and(|b| b.intent.is_some())
    }

    fn alive_character(&self, e: Entity) -> bool {
        self.bodies
            .get(e)
            .is_ok_and(|b| b.intent.is_some() && b.health.is_some_and(|h| h.current > 0.0))
    }

    /// What doesn't move: static bodies (the map) and kinematic ones at
    /// rest (brush entities: a func_brush, a closed door; the logic moves
    /// them by their pose, so their `MovingSolid` says how fast).
    fn still(&self, collider: Entity, body: Entity) -> bool {
        self.rigid_is(collider, body, |rb, solid, v, w| {
            rb.is_static()
                || (rb.is_kinematic()
                    && solid.is_none_or(|m| m.velocity.length_squared() < 1e-8)
                    && v.is_none_or(|v| v.0.length_squared() < 1e-8)
                    && w.is_none_or(|w| w.0.length_squared() < 1e-8))
        })
    }

    /// A static body (the map's own, a frozen prop): a bolt sticks in it
    /// even when it takes damage.
    fn fixed(&self, collider: Entity, body: Entity) -> bool {
        self.rigid_is(collider, body, |rb, _, _, _| rb.is_static())
    }

    fn rigid_is(
        &self,
        collider: Entity,
        body: Entity,
        f: impl Fn(&RigidBody, Option<&MovingSolid>, Option<&LinearVelocity>, Option<&AngularVelocity>) -> bool,
    ) -> bool {
        self.rigid
            .get(collider)
            .or_else(|_| self.rigid.get(body))
            .is_ok_and(|(rb, solid, v, w)| f(rb, solid, v, w))
    }

    /// A sky face within `probe` of `from` along `dir`, nothing solid well
    /// before it.
    fn on_sky(&self, from: Vec3, dir: Dir3, probe: f32, filter: &SpatialQueryFilter) -> bool {
        let Some(at) = self.sky.as_ref().and_then(|s| s.along(from, *dir, probe)) else {
            return false;
        };
        // The sky brush's own face stops the line at `at`.
        const FACE: f32 = 0.02;
        at <= FACE || self.spatial.cast_ray(from, dir, at - FACE, true, filter).is_none()
    }

    fn takes_damage(&self, e: Entity) -> bool {
        self.bodies
            .get(e)
            .is_ok_and(|b| b.health.is_some() || b.damageable.is_some())
    }

    /// The hitgroup a line through character `e` meets first among its
    /// hitboxes (None: it misses them all); without hitboxes, by height.
    fn hitbox_group(&self, e: Entity, from: Vec3, dir: Vec3, len: f32) -> Option<Hitgroup> {
        let b = self.bodies.get(e).ok()?;
        match b.hitboxes.get() {
            Some(boxes) if !boxes.0.is_empty() => {
                let feet = b
                    .aabb
                    .map_or(b.transform.translation, |a| b.transform.translation.with_y(a.min.y));
                let turn = b.intent.map_or(Quat::IDENTITY, |i| i.yaw_rotation()).inverse();
                let (o, d) = (turn * (from - feet), turn * dir);
                boxes.nearest(o, d, len).map(|(i, ..)| boxes.0[i].region.hitgroup())
            }
            // No hitboxes: by where the line enters its box.
            _ => Some(b.aabb.map_or(Hitgroup::Generic, |a| {
                let t = ray_box(a.min, a.max, from, dir, len).unwrap_or(0.0);
                super::hitgroup_at(a, from + dir * t)
            })),
        }
    }

    /// Deal `d` to `target`, hit at `point` going `dir` (unit), in `group`.
    #[allow(clippy::too_many_arguments)]
    fn direct(&mut self, d: &Direct, f: &Flyer, target: Entity, point: Vec3, dir: Vec3, group: Hitgroup, from: Vec3) {
        let character = self.is_character(target);
        let raw = d.damage
            * if character && group == Hitgroup::Head {
                d.head
            } else {
                1.0
            };
        let mut damage = Damage {
            target,
            attacker: f.owner,
            amount: raw,
            point,
            dir,
            hitgroup: if character { group } else { Hitgroup::Generic },
            kind: d.kind,
            weapon: Some(f.weapon),
            force: dir * d.force,
        };
        if character {
            self.armor.apply(&mut damage, raw, d.armor_ratio, d.quantum);
        }
        self.inflicted.write(Inflicted {
            target,
            amount: damage.amount,
            from,
        });
        self.damage.write(damage);
    }
}

/// Flyers whose time ran out, and stuck bolts done.
fn expire(
    flyers: Query<(Entity, &Flyer, &Transform)>,
    clock: Res<SimClock>,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for (e, f, t) in &flyers {
        let due = clock.now + 1e-6;
        let expired = f.expires.is_some_and(|x| due >= x);
        let unstuck = f.stuck_until.is_some_and(|x| due >= x);
        if !expired && !unstuck {
            continue;
        }
        gone.write(FlyerGone {
            flyer: e,
            weapon: f.weapon,
            owner: f.owner,
            at: t.translation,
            expired,
        });
        commands.entity(e).despawn();
    }
}

/// One tick of every flyer: the toss step and its first touch.
fn fly(
    mut flyers: Query<(Entity, &mut Flyer, &mut Transform)>,
    mut world: FlyWorld,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    if dt <= 0.0 {
        return;
    }
    // The dead aren't in the way.
    let dead: Vec<Entity> = world
        .bodies
        .iter()
        .filter(|b| b.intent.is_some() && b.health.is_some_and(|h| h.current <= 0.0))
        .map(|b| b.entity)
        .collect();
    for (e, mut f, mut t) in &mut flyers {
        f.last_hit = None;
        if f.born_tick >= clock.tick || f.stuck_until.is_some() || f.expires.is_some_and(|x| clock.now + 1e-6 >= x) {
            continue;
        }
        f.ticks += 1;
        let (step, v) = gravity_step(f.velocity, f.gravity, dt);
        f.velocity = v;
        let len = step.length();
        let Ok(dir) = Dir3::new(step) else { continue };
        if clock.now < f.solid_after - 1e-6 {
            t.translation += step;
            t.rotation = facing(*dir) * f.turn;
            continue;
        }
        let from = t.translation;
        // The sweep: its shape (or its centre line) against everything
        // solid but its owner, the dead, other flyers (and loose items).
        let mut mask = SOLID_LAYERS.0;
        if f.pass_items {
            mask &= !ITEM_LAYER.0;
        }
        let excluded: Vec<Entity> = f.owner.into_iter().chain(dead.iter().copied()).collect();
        let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(LayerMask(mask));
        let flyers_q = &world.flyers;
        let colliders_q = &world.colliders;
        let not_flyer = |c: Entity| {
            let b = colliders_q.get(c).map_or(c, |o| o.body);
            !flyers_q.contains(b) && !flyers_q.contains(c)
        };
        let mut skip_chars: Vec<Entity> = Vec::new();
        let hit = loop {
            let pred = |c: Entity| {
                let b = colliders_q.get(c).map_or(c, |o| o.body);
                not_flyer(c) && !skip_chars.contains(&b)
            };
            let h = if f.half == Vec3::ZERO {
                world
                    .spatial
                    .cast_ray_predicate(from, dir, len, true, &filter, &pred)
                    .map(|h| (h.distance, h.normal, h.entity))
            } else {
                let shape = if f.sphere {
                    Collider::sphere(f.half.x)
                } else {
                    Collider::cuboid(f.half.x * 2.0, f.half.y * 2.0, f.half.z * 2.0)
                };
                let config = ShapeCastConfig {
                    max_distance: len,
                    ignore_origin_penetration: true,
                    ..ShapeCastConfig::DEFAULT
                };
                world
                    .spatial
                    .cast_shape_predicate(&shape, from, Quat::IDENTITY, dir, &config, &filter, &pred)
                    .map(|h| (h.distance, h.normal1, h.entity))
            };
            // A character with hitboxes is hit only where a hitbox is.
            if let Some((_, _, c)) = h
                && f.hitboxes
            {
                let body = world.body(c);
                if world.alive_character(body) && world.hitbox_group(body, from, *dir, len + 1.0).is_none() {
                    skip_chars.push(body);
                    continue;
                }
            }
            break h;
        };
        let Some((distance, normal, collider)) = hit else {
            t.translation += step;
            if f.velocity.length_squared() > 0.0 {
                t.rotation = facing(f.velocity.normalize()) * f.turn;
            }
            continue;
        };
        let body = world.body(collider);
        let point = from + *dir * distance;
        t.translation = from + *dir * (distance - SKIN).max(0.0);
        // The sky: nothing else of what stopped it is asked.
        if let Some(end) = f.sky_end
            && world.on_sky(t.translation, dir, end.probe, &filter)
        {
            world.gone.write(FlyerGone {
                flyer: e,
                weapon: f.weapon,
                owner: f.owner,
                at: t.translation,
                expired: end.expired,
            });
            commands.entity(e).despawn();
            continue;
        }
        let mut n = normal.normalize_or(-*dir);
        if n.dot(*dir) > 0.0 {
            n = -n;
        }
        let character = world.is_character(body);
        let still = !character && world.still(collider, body);
        let fixed = still && world.fixed(collider, body);
        let velocity = f.velocity;
        f.last_hit = Some(body);
        let group = if character {
            world
                .hitbox_group(body, from, *dir, len + 1.0)
                .unwrap_or(Hitgroup::Generic)
        } else {
            Hitgroup::Generic
        };
        let damageable = world.takes_damage(body);
        let mut gone = false;
        match f.touch.clone() {
            Touch::Report => {
                f.velocity = Vec3::ZERO;
            }
            Touch::Explode => {
                gone = true;
                if let Some(b) = f.blast.clone() {
                    world.blasts.write(BlastAt {
                        at: t.translation,
                        blast: b,
                        thrower: f.owner,
                        weapon: f.weapon,
                        inflictor: Some(e),
                    });
                }
            }
            Touch::Bounce => {
                if let Some(d) = f.direct
                    && damageable
                    && (!character || world.alive_character(body))
                {
                    world.direct(&d, &f, body, point, *dir, group, t.translation);
                }
                let speed = f.keep_speed.unwrap_or(velocity.length()).max(velocity.length());
                f.velocity = reflect(velocity, n).normalize_or(n) * speed;
            }
            Touch::Bolt(rule) => {
                // What takes damage takes the bolt (a breakable, a
                // prop; a kinematic one at rest too), unless the map's own.
                if damageable && !fixed {
                    if let Some(d) = f.direct {
                        world.direct(&d, &f, body, point, *dir, group, t.translation);
                    }
                    gone = true;
                } else {
                    let speed = velocity.length();
                    let c = n.dot(-velocity / speed.max(1e-6));
                    if still && c < rule.glance_cos && speed > rule.glance_speed {
                        f.velocity = reflect(velocity, n) * rule.glance_keep;
                        f.gravity = rule.glance_gravity;
                    } else if still {
                        f.velocity = Vec3::ZERO;
                        f.stuck_until = Some(clock.now + rule.stick_time as f64);
                    } else {
                        gone = true;
                    }
                }
            }
        }
        if f.velocity.length_squared() > 0.0 {
            t.rotation = facing(f.velocity.normalize()) * f.turn;
        }
        world.hits.write(FlyerHit {
            flyer: e,
            weapon: f.weapon,
            owner: f.owner,
            other: (!still).then_some(body),
            point,
            normal: n,
            velocity,
            character,
            world: still,
            gone,
        });
        if gone {
            world.gone.write(FlyerGone {
                flyer: e,
                weapon: f.weapon,
                owner: f.owner,
                at: t.translation,
                expired: false,
            });
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_enter_boxes_and_spheres() {
        let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
        assert_eq!(ray_box(lo, hi, Vec3::new(-3.0, 0.0, 0.0), Vec3::X, 10.0), Some(2.0));
        assert_eq!(ray_box(lo, hi, Vec3::ZERO, Vec3::X, 10.0), Some(0.0));
        assert_eq!(ray_box(lo, hi, Vec3::new(-3.0, 2.0, 0.0), Vec3::X, 10.0), None);
        assert_eq!(ray_box(lo, hi, Vec3::new(-3.0, 0.0, 0.0), Vec3::X, 1.0), None);
        let t = ray_sphere(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, 1.0).unwrap();
        assert!((t - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(ray_sphere(Vec3::ZERO, Vec3::X, Vec3::ZERO, 1.0), Some(0.0));
    }

    #[test]
    fn a_source_model_noses_along_its_flight() {
        let r = facing(Vec3::Z) * MODEL_X_FORWARD;
        assert!((r * Vec3::X - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn a_y_nosed_model_noses_along_its_flight() {
        for d in [Vec3::Z, Vec3::X, Vec3::new(0.3, 0.5, -0.8).normalize()] {
            let r = facing(d) * MODEL_Y_FORWARD;
            assert!((r * Vec3::Y - d).length() < 1e-5);
        }
    }

    #[test]
    fn a_model_faces_its_flight() {
        let r = facing(Vec3::X);
        assert!((r * Vec3::NEG_Z - Vec3::X).length() < 1e-5);
        let up = facing(Vec3::Y);
        assert!((up * Vec3::NEG_Z - Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn data_units_become_meters() {
        let p: LauncherParams = toml::from_str(
            "speed = 3500.0\nwater_speed = 1500.0\ngravity = 30.0\nhalf = [1.0, 1.0, 1.0]\ntouch = \"bolt\"\nglance_cos = 0.5\nglance_speed = 100.0\nglance_keep = 0.75\nglance_gravity = 600.0\nstick_time = 2.0\ndamage = 100.0\nhead = 3.0\nforce = 21000.0",
        )
        .unwrap();
        let l = p.launcher("x", 0.0254, 0.01);
        assert!((l.speed - 88.9).abs() < 1e-3);
        assert!((l.template.gravity - 0.762).abs() < 1e-5);
        let d = l.template.direct.unwrap();
        assert!((d.damage - 1.0).abs() < 1e-6);
        assert_eq!(d.kind, DamageKind::Bullet);
        assert!(matches!(l.template.touch, Touch::Bolt(r) if (r.glance_speed - 2.54).abs() < 1e-4));
    }
}
