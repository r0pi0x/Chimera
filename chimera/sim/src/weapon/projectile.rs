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
//! hitboxes; with `Flyer::hull_boxes` a character with a movement hull
//! is met as that box, not its collider: TF2's). Games steer flyers (guidance, seeking) in `FlyerSet::Think`
//! and answer `FlyerHit`s in `FlyerSet::Touch`. Movement is Source's toss
//! (`grenade::gravity_step`, exact for constant gravity) and a swept shape
//! against `core::SOLID_LAYERS`, never touching its owner (nor loose
//! items with `Flyer::pass_items`, nor anything before
//! `Flyer::solid_after`), and physics clips with `Flyer::meets_clips`; a flyer made during a tick first moves on the
//! next; one with `Flyer::drag` loses speed and spin to the air after
//! gravity each tick, and `LauncherParams::spin` tumbles it from its
//! launch (`flight`). The server's (`core::authoritative`); clients draw it from the
//! network's `NetItem` (its `ShownItem` and `Transform`, facing its
//! flight with its model's `Flyer::turn`, or tumbling by its
//! `Flyer::spin`: `flyer_rotation`), as thrown grenades. The turn comes
//! from its shown model's nose as it is made (`face_nose`, for every game:
//! the model's own forward, however its loader's frame turned it), so no
//! weapon names its model's axes.
//!
//! Made by a game's `LaunchRequest` (an alt-fire, a rocket) or from a
//! weapon's frame rounds: a weapon with `chimera:projectile`
//! (`LauncherParams`, in the data's units; `Launcher`) turns each of its
//! `Fired` rounds into a flyer from the eye along the shot's angles
//! (`Fired` without a `DamageEffect`: the frame traces nothing), at
//! `water_speed` when its owner is marked `LaunchUnderWater`; optionally
//! toward the point its aim line meets (`aim_trace`, `aim_min`), from a
//! start the world pulls back (`start_trace`), turned by a random
//! `jitter` (TF2's rocket and needle).
//!
//! Also here, shared with Combat Arms' special weapons (whose objects run
//! on their own 20 Hz frames): the ray tests `ray_box` and `ray_sphere`,
//! `box_face`, and `facing` (the rotation a model flying along a direction is drawn
//! with).
//!
//! Units: meters, seconds, damage normalized like `Health`.
//!
//! A flyer with `Flyer::marks` (`marks` in the data: TF2's needle) marks
//! what it touches but characters as a bullet's impact would: the server's
//! `FlyerMark` (sent to network clients), shown on every machine that
//! draws when the drawn time reaches the touch (`map::loose::LateDelay`
//! after it) as a `WeaponEventKind::Marked`, the touched surface found
//! again in that machine's world (`show_marks`); the games' impact effects
//! draw it as a bullet's hit (decal, debris, the surface's sound). It
//! waits for the drawn time only where the flyer is drawn late (a network
//! client's snapshots, `map::loose::DrawnLate`), else it shows at once.
//! With `Flyer::marks_stay` its model stays there, still, for the rest of
//! its life (`StuckMark`, made by each machine showing the mark: TF2's
//! needle stuck in a wall until 6 s from its launch, effects.md 12).
//!
//! A flyer with `Flyer::sky_end` ends on the sky (a line that far along its
//! flight meets `map::MapSkyFaces` with nothing before it): gone, no touch
//! rule, its `FlyerGone::expired` as said.
//!
//! A bolt with `BoltRule::through_glass` flies on through breakable glass
//! (what takes damage and lets light through: `map::SeeThrough`), dealing
//! it the direct damage and never meeting it again (`Flyer::passed`); its
//! `FlyerHit` there is neither the world nor its end. A flyer with
//! `Flyer::pass_grates` passes the map's grates (`map::Grate`).
//!
//! Known shortcuts: a bounce, a glance or a pass through glass spends the
//! rest of its tick's move; the
//! hitbox test of `Flyer::hitboxes` runs on the swept line's segment
//! through the character's hull (a hit on the hull that misses every
//! hitbox lets it fly on); grates are only the world's own (`MapData::
//! grate_hulls`: a brush entity's grate volumes stop a bolt); a
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
    core::{
        Damage, DamageKind, Damageable, Health, Hitgroup, ITEM_LAYER, Intent, MovingSolid, PHYSICS_CLIP_LAYER, SOLID_LAYERS,
        SimClock,
    },
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
    /// Breakable glass (what takes damage and lets light through,
    /// `map::SeeThrough`) takes its direct damage and the bolt flies on
    /// (projectiles.md 7.4).
    pub through_glass: bool,
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
    /// It passes the map's grates (`map::Grate`).
    pub pass_grates: bool,
    /// What it has passed through (breakable glass, `BoltRule::
    /// through_glass`): not met again.
    pub passed: Vec<Entity>,
    /// A physics object: physics clips (`map::physics_clip`, those whose
    /// filter passes it) stop it as they stop props.
    pub meets_clips: bool,
    /// Characters with hitboxes are hit only where a hitbox is.
    pub hitboxes: bool,
    /// A character with a movement hull (`MovementState::hull_min`..
    /// `hull_max`: Source's, Combat Arms') is met as that box, not its
    /// collider (TF2's projectiles meet players' collision boxes,
    /// specs/tf2/weapons.md Open questions 12).
    pub hull_boxes: bool,
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
    /// along it): set as it is made from its shown model's nose
    /// (`map::loose::LooseAsset::nose_turn`, `face_nose`), whatever its
    /// game or its model's axes; identity when the nose isn't known.
    pub turn: Quat,
    /// A tumbling flyer's spin, rad/s about its own axes (HL2's SMG1
    /// grenade): it faces its launch once (`turn` too) and then turns
    /// by this instead of following its flight. Zero: it faces its
    /// flight.
    pub spin: Vec3,
    /// Its air drag (`flight::AirDrag`: speed and spin lost each tick
    /// after gravity, by its game's `flight::AirDensity`); none: none.
    pub drag: Option<super::flight::AirDrag>,
    /// It passes its owner's teammates (characters on the owner's
    /// non-zero `core::Team`, friendly fire off: TF2's rockets).
    pub pass_teammates: bool,
    /// Its touch on anything but a character marks it as a bullet's
    /// impact would (`FlyerMark`: TF2's needle).
    pub marks: bool,
    /// Its model stays where it marked the world until its life from the
    /// launch runs out (`FlyerMark::stay`: TF2's needle, effects.md 12).
    pub marks_stay: bool,
}

/// How a flyer ends on the sky: the line ahead that finds it (m), and
/// whether `FlyerGone` says its time ran out (a ball's harmless end) or not
/// (silently gone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyEnd {
    pub probe: f32,
    pub expired: bool,
}

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
            pass_grates: false,
            passed: Vec::new(),
            meets_clips: false,
            hitboxes: false,
            hull_boxes: false,
            touch: Touch::Report,
            direct: None,
            blast: None,
            stuck_until: None,
            sky_end: None,
            last_hit: None,
            turn: Quat::IDENTITY,
            spin: Vec3::ZERO,
            drag: None,
            pass_teammates: false,
            marks: false,
            marks_stay: false,
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

/// A flyer with `Flyer::marks` touched something not a character (the
/// server's, sent to network clients): its weapon, owner, the touch point
/// `at` (m), the surface's `normal` and its flight's direction `dir`; its
/// owner's team (0: none); its model's turn there and how long it stays
/// there (s; 0: not, `Flyer::marks_stay`).
/// Every machine that draws shows it (`show_marks`) as a
/// `WeaponEventKind::Marked` impact when its drawn time reaches the touch
/// (`map::loose::LateDelay` after it: the needle drawn late reaches the
/// wall then, specs/tf2/effects.md 11).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct FlyerMark {
    pub weapon: &'static str,
    pub owner: Option<Entity>,
    pub at: Vec3,
    pub normal: Vec3,
    pub dir: Vec3,
    pub team: u8,
    pub turn: Quat,
    pub stay: f32,
}

/// How far back along its flight, and past the touch, a mark's surface
/// is looked for on the machine showing it (m).
const MARK_REACH: f32 = 0.1;

/// The marks of `FlyerMark`s whose drawn time has come, as
/// `WeaponEventKind::Marked` weapon events (every machine; the impact
/// effects draw them): what this machine's world has at the touch point
/// (a short line along the flight), not the server's entity.
#[allow(clippy::too_many_arguments)]
fn show_marks(
    time: Res<Time>,
    delay: Option<Res<crate::map::loose::LateDelay>>,
    (late, staying, role): (Option<Res<crate::map::loose::DrawnLate>>, Option<Res<StayingMarks>>, Option<Res<crate::core::NetRole>>),
    mut marks: MessageReader<FlyerMark>,
    mut held: Local<Vec<(f64, FlyerMark)>>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    characters: Query<(), With<Intent>>,
    mut events: MessageWriter<WeaponEvent>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    // Shown as its flyer is drawn there: a delay late on a network client
    // (from snapshots) or when drawn late (`DrawnLate`), else at once.
    let client = role.as_deref() == Some(&crate::core::NetRole::Client);
    let wait = |m: &FlyerMark| {
        let late_here = client || late.as_deref().is_some_and(|l| l.running && l.weapons.contains(m.weapon));
        if late_here { delay.as_deref().map_or(0.0, |d| d.0 as f64) } else { 0.0 }
    };
    held.extend(marks.read().map(|m| (now + wait(m), *m)));
    if held.is_empty() {
        return;
    }
    let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
    held.retain(|(due, m)| {
        if *due > now + 1e-9 {
            return true;
        }
        let Ok(dir) = Dir3::new(m.dir) else { return false };
        let from = m.at - *dir * MARK_REACH;
        let hit = spatial.cast_ray_predicate(from, dir, 2.0 * MARK_REACH, true, &filter, &|e| {
            !characters.contains(colliders.get(e).map_or(e, |o| o.body))
        });
        let Some(hit) = hit else { return false };
        let body = colliders.get(hit.entity).map_or(hit.entity, |o| o.body);
        // Its model stays there, still, for the rest of its life.
        if m.stay > 0.0
            && let Some(key) = staying.as_deref().and_then(|s| s.0.get(m.weapon))
        {
            commands.spawn((
                Name::new(format!("Stuck ({key})")),
                crate::map::loose::ShownItem(key.clone()),
                StuckMark { until: now + m.stay as f64 },
                Transform::from_translation(m.at).with_rotation(m.turn),
                Visibility::default(),
            ));
        }
        events.write(WeaponEvent {
            owner: m.owner.unwrap_or(Entity::PLACEHOLDER),
            weapon: Entity::PLACEHOLDER,
            kind: WeaponEventKind::Marked {
                id: m.weapon,
                from,
                to: from + *dir * hit.distance,
                hit: body,
                normal: hit.normal,
                team: m.team,
            },
            replay: false,
        });
        false
    });
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
    /// Speed added along the aim's up (data length/s): TF2's grenades
    /// leave at 1200 forward and 200 up (specs/tf2/weapons_more.md 4.1).
    #[serde(default)]
    pub up_speed: f32,
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
    /// Characters are met as their movement hull's box (`Flyer::hull_boxes`).
    #[serde(default)]
    pub hull_boxes: bool,
    #[serde(default)]
    pub pass_items: bool,
    /// It passes the map's grates (`Flyer::pass_grates`).
    #[serde(default)]
    pub pass_grates: bool,
    /// A bolt flies on through breakable glass (`BoltRule::through_glass`).
    #[serde(default)]
    pub through_glass: bool,
    /// Lifetime, seconds.
    #[serde(default)]
    pub life: Option<f32>,
    /// Ends on the sky a line this long finds (data length; 0: never).
    #[serde(default)]
    pub sky_probe: f32,
    /// Where it starts from the eye, along the aim's forward, right and up
    /// (data lengths; none: the eye), and crouched (none: `offset`):
    /// TF2's rocket under the gun (specs/tf2/weapons.md 5.1).
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    #[serde(default)]
    pub offset_crouched: Option<[f32; 3]>,
    /// It passes its owner's teammates (`Flyer::pass_teammates`).
    #[serde(default)]
    pub pass_teammates: bool,
    /// It heads from its start for the point a line this long (data
    /// length; 0: along the aim) from the eye along the aim meets (the
    /// world or a character but its owner and his teammates), else for the
    /// line's end; a meeting nearer than `aim_min` counts as none (TF2's
    /// rocket: 2000 and 200, specs/tf2/weapons.md 5.1; its needle: 2000).
    #[serde(default)]
    pub aim_trace: f32,
    #[serde(default)]
    pub aim_min: f32,
    /// Its start pulled back to where a line from the eye to it meets the
    /// world (TF2's rocket).
    #[serde(default)]
    pub start_trace: bool,
    /// A random turn of its flight, uniform within +- this many degrees in
    /// pitch and in yaw, from the shot's seed (TF2's needle: 1.5).
    #[serde(default)]
    pub jitter: f32,
    /// Drawn in the past on every machine that draws, by the
    /// interpolation delay, the shooter's own too (TF2's projectiles,
    /// specs/tf2/effects.md 11): its `shown` key joins
    /// `map::loose::DrawnLate` (`chimera_net`'s `late`).
    #[serde(default)]
    pub interp: bool,
    /// Drawn late, its model hidden its first this many seconds from its
    /// arrival (its trail shows): TF2's rocket 0.2, its pipes 0.1
    /// (effects.md 11.1).
    #[serde(default)]
    pub model_hidden: f32,
    /// Drawn late, its first received place rounded to whole units of
    /// the data (Source sends a projectile's origin in whole inches:
    /// specs/tf2/effects.md 11.2).
    #[serde(default)]
    pub whole_units: bool,
    /// Its touch on anything but a character leaves a bullet's impact
    /// there (the surface's decal, debris and sound, `FlyerMark`): TF2's
    /// needle (specs/tf2/effects.md, "Needles on a wall": a hole mark and
    /// splinters, no needle left standing).
    #[serde(default)]
    pub marks: bool,
    /// Its model stays, still, where it marked the world, until its `life`
    /// from the launch runs out (`StuckMark`; TF2's needle: 6 s,
    /// effects.md 12).
    #[serde(default)]
    pub marks_stay: bool,
    /// Its launch spin (`flight::LaunchSpin`, from the shot's seed;
    /// TF2's grenade); none: it faces its flight.
    #[serde(default)]
    pub spin: Option<super::flight::LaunchSpin>,
    /// Its air drag (`flight::DragParams`, in the data's lengths).
    #[serde(default)]
    pub drag: Option<super::flight::DragParams>,
}

/// The weapons of `set` whose projectiles are drawn late
/// (`LauncherParams::interp`): (weapon id, `ShownItem` key, how long its
/// model stays hidden: `model_hidden`, the unit in metres its first
/// received place is rounded to, 0 for none: `whole_units`).
pub fn late_keys(set: &super::data::WeaponSet) -> Vec<(String, String, f32, f32)> {
    set.weapons
        .iter()
        .filter_map(|w| {
            let p: LauncherParams = w.hooks.get(LAUNCHER)?.clone().try_into().ok()?;
            p.interp.then(|| (w.id.clone(), p.shown.clone().unwrap_or_else(|| w.id.clone()), p.model_hidden, if p.whole_units { w.units.length } else { 0.0 }))
        })
        .collect()
}

/// The weapons whose projectiles' models stay where they marked the world
/// (`LauncherParams::marks_stay`), by weapon id: their `ShownItem` keys.
#[derive(Resource, Clone, Debug, Default)]
pub struct StayingMarks(pub std::collections::BTreeMap<String, String>);

impl StayingMarks {
    /// `set`'s.
    pub fn add_set(&mut self, set: &super::data::WeaponSet) {
        for w in &set.weapons {
            let Some(p) = w.hooks.get(LAUNCHER).and_then(|p| p.clone().try_into::<LauncherParams>().ok()) else { continue };
            if p.marks && p.marks_stay {
                self.0.insert(w.id.clone(), p.shown.clone().unwrap_or_else(|| w.id.clone()));
            }
        }
    }
}

/// A projectile's model left where it marked the world (`StayingMarks`),
/// on the machine that shows the mark (never sent): gone at `until`
/// (this machine's time, s).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct StuckMark {
    pub until: f64,
}

/// Stuck models whose time ran out go.
fn end_stuck_marks(time: Res<Time>, q: Query<(Entity, &StuckMark)>, mut commands: Commands) {
    let now = time.elapsed_secs_f64();
    for (e, m) in &q {
        if now >= m.until {
            commands.entity(e).despawn();
        }
    }
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
    /// Added along the aim's up, m/s (`LauncherParams::up_speed`).
    pub up_speed: f32,
    pub water_speed: Option<f32>,
    pub shown: String,
    pub template: Flyer,
    /// Its start from the eye along the aim's forward, right and up, m,
    /// standing and crouched (`LauncherParams::offset`).
    pub offset: Vec3,
    pub offset_crouched: Vec3,
    /// `LauncherParams::aim_trace`, `aim_min` (m), `start_trace`,
    /// `jitter` (radians).
    pub aim_trace: f32,
    pub aim_min: f32,
    pub start_trace: bool,
    pub jitter: f32,
    /// `LauncherParams::spin`.
    pub spin: Option<super::flight::LaunchSpin>,
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
        f.pass_grates = self.pass_grates;
        f.sky_end = (self.sky_probe > 0.0).then_some(SkyEnd {
            probe: self.sky_probe * unit,
            expired: false,
        });
        f.hitboxes = self.hitboxes;
        f.hull_boxes = self.hull_boxes;
        f.pass_teammates = self.pass_teammates;
        f.marks = self.marks;
        f.marks_stay = self.marks_stay;
        f.drag = self.drag.map(|d| d.drag(unit));
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
                through_glass: self.through_glass,
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
            up_speed: self.up_speed * unit,
            water_speed: self.water_speed.map(|s| s * unit),
            shown: self.shown.clone().unwrap_or_else(|| weapon.to_string()),
            template: f,
            offset: self.offset.map_or(Vec3::ZERO, |o| Vec3::from(o) * unit),
            offset_crouched: self.offset_crouched.or(self.offset).map_or(Vec3::ZERO, |o| Vec3::from(o) * unit),
            aim_trace: self.aim_trace * unit,
            aim_min: self.aim_min * unit,
            start_trace: self.start_trace,
            jitter: self.jitter.to_radians(),
            spin: self.spin,
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
        .add_message::<FlyerMark>()
        .add_message::<WeaponEvent>()
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
        .add_observer(face_nose)
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
        )
        // Every machine (a client's marks come from the server), each
        // frame: the impact effects read its weapon events on the next
        // tick (in `Update`, off the ticks' schedules, which the
        // server-client ordering ratchet keeps).
        .init_resource::<StayingMarks>()
        .add_systems(Update, (show_marks, end_stuck_marks));
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

/// The outward normal of the face of box `min`..`max` that `p` (on it)
/// lies nearest: where `ray_box` entered it.
pub fn box_face(min: Vec3, max: Vec3, p: Vec3) -> Vec3 {
    let (mut n, mut gap) = (Vec3::Y, f32::MAX);
    for i in 0..3 {
        for (face, sign) in [(min[i], -1.0), (max[i], 1.0)] {
            if (p[i] - face).abs() < gap {
                gap = (p[i] - face).abs();
                n = Vec3::ZERO;
                n[i] = sign;
            }
        }
    }
    n
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

/// Flyer `f`'s rotation after a step of `dt` s along `dir` (unit): facing
/// its flight with its model's turn, or a tumbling one's (`Flyer::spin`)
/// turned on from `now` by its spin.
pub fn flyer_rotation(f: &Flyer, now: Quat, dir: Vec3, dt: f32) -> Quat {
    if f.spin != Vec3::ZERO {
        (now * Quat::from_scaled_axis(f.spin * dt)).normalize()
    } else {
        facing(dir) * f.turn
    }
}

/// The reflection of `v` on a surface of normal `n`.
pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - 2.0 * v.dot(n) * n
}

// ---------------------------------------------------------------------------
// Making flyers

/// A new flyer's model turn from its shown model's nose (`Flyer::turn`),
/// and its rotation with it: every flyer of every game flies nose first,
/// its model's own forward (`MapHeldModel::nose`: a Source model's +X in
/// whatever frame its loader left it) along its velocity. Without the
/// model (a headless test's greybox) or its nose, the turn is left as made.
fn face_nose(
    add: On<Add<Flyer>>,
    mut flyers: Query<(&mut Flyer, &ShownItem, &mut Transform)>,
    assets: Option<Res<crate::map::loose::LooseAssets>>,
) {
    let Some(assets) = assets else { return };
    let Ok((mut f, shown, mut t)) = flyers.get_mut(add.entity) else { return };
    let Some(a) = assets.0.get(&shown.0).filter(|a| a.nose.is_some()) else { return };
    let turn = a.nose_turn();
    // Its rotation as made, the old turn taken off: a tumbling one keeps
    // its launch's facing.
    t.rotation = (t.rotation * f.turn.inverse() * turn).normalize();
    f.turn = turn;
}

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
/// a flyer from the eye along the round's angles (turned by its `jitter`),
/// or toward its aim line's point (`aim_trace`), from its offset (pulled
/// back by the world: `start_trace`).
#[allow(clippy::too_many_arguments)]
fn launch_from_fired(
    mut events: MessageReader<WeaponEvent>,
    launchers: Query<&Launcher>,
    under: Query<(), With<LaunchUnderWater>>,
    stances: Query<&crate::core::MovementState>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    characters: Query<Option<&crate::core::Team>, With<Intent>>,
    friendly_fire: Option<Res<crate::core::FriendlyFire>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for e in events.read() {
        let WeaponEventKind::Fired { origin, yaw, pitch, seed, .. } = e.kind else {
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
        let aim = super::aim_of(yaw, pitch);
        let mut dir = aim * Vec3::NEG_Z;
        // Its start: the eye, moved along the aim's forward, right and up.
        let crouched = stances.get(e.owner).is_ok_and(|s| s.crouching);
        let o = if crouched { l.offset_crouched } else { l.offset };
        let mut start = origin + dir * o.x + aim * Vec3::X * o.y + aim * Vec3::Y * o.z;
        let solid = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let body = |c: Entity| colliders.get(c).map_or(c, |o| o.body);
        if l.start_trace
            && let Ok(to) = Dir3::new(start - origin)
            && let Some(hit) = spatial.cast_ray_predicate(origin, to, start.distance(origin), true, &solid, &|c| {
                !characters.contains(body(c))
            })
        {
            start = origin + to * hit.distance;
        }
        if l.aim_trace > 0.0 {
            let ff = friendly_fire.as_ref().is_some_and(|f| f.0 != 0);
            let mine = characters.get(e.owner).ok().flatten().copied().filter(|t| t.0 != 0);
            let passes = |c: Entity| {
                let b = body(c);
                b == e.owner || (!ff && mine.is_some() && characters.get(b).ok().flatten().copied() == mine)
            };
            let to = match spatial.cast_ray_predicate(origin, Dir3::new(dir).unwrap_or(Dir3::NEG_Z), l.aim_trace, true, &solid, &|c| {
                !passes(c)
            }) {
                Some(hit) if hit.distance > l.aim_min => origin + dir * hit.distance,
                _ => origin + dir * l.aim_trace,
            };
            dir = (to - start).normalize_or(dir);
        }
        if l.jitter > 0.0 {
            let mut rng = super::random::Rng::new(seed ^ 0x6a17);
            let (dp, dy) = (rng.range(-l.jitter, l.jitter), rng.range(-l.jitter, l.jitter));
            let (up, right) = (aim * Vec3::Y, aim * Vec3::X);
            dir = Quat::from_axis_angle(up, -dy) * (Quat::from_axis_angle(right, dp) * dir);
        }
        let mut f = l.template.clone();
        f.owner = Some(e.owner);
        f.velocity = dir * speed + aim * Vec3::Y * l.up_speed;
        if let Some(spin) = &l.spin {
            f.spin = spin.draw(seed);
        }
        if let Some(life) = f.expires {
            f.expires = Some(clock.now + life);
        }
        spawn_flyer(&mut commands, f, start, l.shown.clone(), &clock);
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
    state: Option<&'static crate::core::MovementState>,
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
    clips: Query<'w, 's, (&'static crate::map::physics_clip::ClipPasses, Has<ColliderDisabled>)>,
    see_through: Query<'w, 's, (), With<crate::map::SeeThrough>>,
    grates: Query<'w, 's, (), With<crate::map::Grate>>,
    teams: Query<'w, 's, (Entity, &'static crate::core::Team), With<Intent>>,
    friendly_fire: Option<Res<'w, crate::core::FriendlyFire>>,
    armor: HitArmor<'w, 's>,
    damage: MessageWriter<'w, Damage>,
    inflicted: MessageWriter<'w, Inflicted>,
    hits: MessageWriter<'w, FlyerHit>,
    marks: MessageWriter<'w, FlyerMark>,
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

    /// A character's movement hull box (m), when it has one.
    fn hull(&self, e: Entity) -> Option<(Vec3, Vec3)> {
        let b = self.bodies.get(e).ok().filter(|b| b.intent.is_some())?;
        let s = b.state.filter(|s| s.hull_max != s.hull_min)?;
        let (lo, hi) = (b.transform.translation + s.hull_min, b.transform.translation + s.hull_max);
        Some((lo.min(hi), hi.max(lo)))
    }

    /// The nearest living character's hull box (`hull`) a box of `half`
    /// (m; zero: a point) swept from `from` along `dir` for `len` meets,
    /// leaving out what `filter` excludes and `skip`s: (its distance, the
    /// face's normal, the character).
    fn hull_hit(&self, from: Vec3, dir: Vec3, len: f32, half: Vec3, filter: &SpatialQueryFilter, skip: [&[Entity]; 2]) -> Option<(f32, Vec3, Entity)> {
        let mut best: Option<(f32, Vec3, Entity)> = None;
        for b in &self.bodies {
            if b.health.is_some_and(|h| h.current <= 0.0) || filter.excluded_entities.contains(&b.entity) || skip.iter().any(|s| s.contains(&b.entity)) {
                continue;
            }
            let Some((lo, hi)) = self.hull(b.entity) else { continue };
            let (lo, hi) = (lo - half, hi + half);
            let Some(t) = ray_box(lo, hi, from, dir, len) else { continue };
            if best.is_some_and(|(d, ..)| d <= t) {
                continue;
            }
            best = Some((t, box_face(lo, hi, from + dir * t), b.entity));
        }
        best
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
    air: Option<Res<super::flight::AirDensity>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    // No flyer (most ticks): no list of the dead to make.
    if dt <= 0.0 || flyers.is_empty() {
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
        let (step, mut v) = gravity_step(f.velocity, f.gravity, dt);
        // Air drag after gravity (flight.rs).
        if let Some(d) = f.drag {
            let rho = air.as_deref().map_or(1.0, |a| a.of(f.weapon));
            (v, f.spin) = d.step(v, f.spin, t.rotation, rho, dt);
        }
        f.velocity = v;
        let len = step.length();
        let Ok(dir) = Dir3::new(step) else { continue };
        if clock.now < f.solid_after - 1e-6 {
            t.translation += step;
            t.rotation = flyer_rotation(&f, t.rotation, *dir, dt);
            continue;
        }
        let from = t.translation;
        // The sweep: its shape (or its centre line) against everything
        // solid but its owner, the dead, other flyers (and loose items).
        let mut mask = SOLID_LAYERS.0;
        if f.pass_items {
            mask &= !ITEM_LAYER.0;
        }
        if f.meets_clips {
            mask |= PHYSICS_CLIP_LAYER.0;
        }
        // Its owner's teammates, when it passes them (friendly fire off;
        // team 0 is nobody's).
        let ff = world.friendly_fire.as_deref().is_some_and(|x| x.0 != 0);
        let owner_team = f.owner.and_then(|o| world.teams.get(o).ok()).map(|(_, t)| *t).filter(|t| t.0 != 0);
        let mates: Vec<Entity> = match owner_team {
            Some(team) if f.pass_teammates && !ff => world.teams.iter().filter(|(_, t)| **t == team).map(|(e, _)| e).collect(),
            _ => Vec::new(),
        };
        let excluded: Vec<Entity> = f
            .owner
            .into_iter()
            .chain(dead.iter().copied())
            .chain(f.passed.iter().copied())
            .chain(mates)
            .collect();
        let filter = SpatialQueryFilter::from_excluded_entities(excluded).with_mask(LayerMask(mask));
        let flyers_q = &world.flyers;
        let colliders_q = &world.colliders;
        let clips_q = &world.clips;
        let grates_q = &world.grates;
        let pass_grates = f.pass_grates;
        let hull_boxes = f.hull_boxes;
        let passed = &f.passed;
        let not_flyer = |c: Entity| {
            let b = colliders_q.get(c).map_or(c, |o| o.body);
            !flyers_q.contains(b) && !flyers_q.contains(c)
        };
        // A filtered clip stops only what its filter passes.
        let clip_stops = |c: Entity| {
            clips_q
                .get(c)
                .ok()
                .is_none_or(|(passes, off)| !off && passes.0.as_ref().is_none_or(|p| p.contains(&e)))
        };
        let mut skip_chars: Vec<Entity> = Vec::new();
        let hit = loop {
            let pred = |c: Entity| {
                let b = colliders_q.get(c).map_or(c, |o| o.body);
                not_flyer(c)
                    && clip_stops(c)
                    && !skip_chars.contains(&b)
                    && !passed.contains(&b)
                    && !(pass_grates && (grates_q.contains(c) || grates_q.contains(b)))
                    && !(hull_boxes && world.hull(b).is_some())
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
            // Characters with a hull met as its box (`Flyer::hull_boxes`).
            let h = match hull_boxes.then(|| world.hull_hit(from, *dir, len, f.half, &filter, [&skip_chars, passed])).flatten() {
                Some(b) if h.is_none_or(|a| b.0 < a.0) => Some(b),
                _ => h,
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
                t.rotation = flyer_rotation(&f, t.rotation, f.velocity.normalize(), dt);
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
        // Breakable glass a bolt flies on through: reported as neither the
        // world nor its end.
        let mut through = false;
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
                // prop; a kinematic one at rest too), unless the map's own;
                // breakable glass takes it and lets it fly on.
                if rule.through_glass
                    && damageable
                    && !character
                    && (world.see_through.contains(collider) || world.see_through.contains(body))
                {
                    if let Some(d) = f.direct {
                        world.direct(&d, &f, body, point, *dir, group, t.translation);
                    }
                    f.passed.push(body);
                    through = true;
                } else if damageable && !fixed {
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
            t.rotation = flyer_rotation(&f, t.rotation, f.velocity.normalize(), dt);
        }
        if f.marks && !character && !through {
            let stay = match (f.marks_stay, f.expires) {
                (true, Some(x)) => (x - clock.now).max(0.0) as f32,
                _ => 0.0,
            };
            world.marks.write(FlyerMark {
                weapon: f.weapon,
                owner: f.owner,
                at: point,
                normal: n,
                dir: *dir,
                team: f.owner.and_then(|o| world.teams.get(o).ok()).map_or(0, |(_, t)| t.0),
                turn: t.rotation,
                stay,
            });
        }
        world.hits.write(FlyerHit {
            flyer: e,
            weapon: f.weapon,
            owner: f.owner,
            other: (!still || through).then_some(body),
            point,
            normal: n,
            velocity,
            character,
            world: still && !through,
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
    fn a_model_noses_along_its_flight_whatever_its_axes() {
        for nose in [Vec3::X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z, Vec3::new(0.2, -0.7, 0.4).normalize()] {
            let a = crate::map::loose::LooseAsset { frame: Transform::IDENTITY, half: Vec3::ONE, surface: None, nose: Some(nose), attachments: Vec::new() };
            for d in [Vec3::Z, Vec3::X, Vec3::Y, Vec3::new(0.3, 0.5, -0.8).normalize()] {
                let r = facing(d) * a.nose_turn();
                assert!((r * nose - d).length() < 1e-5, "{nose} along {d}: {}", r * nose);
            }
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
