//! Halo's weapons in the engine's tick (`super::HaloWeaponsPlugin`): the
//! holders' 30 Hz updates in the spec's per-tick order (inputs, a switch's
//! put-away and ready, the grenade throw's state, the player melee, then
//! every carried weapon's `HaloGun::tick`), the shots' projectiles
//! (created and first moved on the firing tick, on the server), the
//! flight of the ones already out (`fly`), their hits through the damage
//! seam (`HaloHit::direct`, `HaloHit::melee`, `HaloHit::area`) and what
//! presentation reads (`WeaponEvent`s, `HaloFired`, `HaloImpact`,
//! `HaloDetonation`, `HaloStuck`, the zoom as `Zoomed`).
//!
//! A projectile (`HaloProjectile`) carries its tag's numbers (a weapon's
//! trigger's, or a grenade's), its owner, its flight, its guidance target
//! (a needle's and the charged bolt's: the autoaim's target of the shot,
//! weapons.md 15 and `aim`; turned by the guided rate / 30, half
//! against a player's unit, toward the target's centre moved by the
//! wander), and what holds it: stuck in a character (it moves with it;
//! a needle sticking resets the other unconsumed needles of the same tag
//! in that character) or in a thrower's hand (only its timers run). Its
//! end: a disappearance deletes it; a detonation (`detonate`) checks the
//! needles' supercombine (more than 6 unconsumed needles of the tag in a
//! biped: the others consumed or reset, this one detached to the biped's
//! feet with the super detonation), deals the detonation effect's area
//! damage and pushes (`explode`; a character's push as `HaloPush`, applied
//! by `apply_pushes` through `movement::push`; a grenade's to its flight),
//! a stuck one's attached damage on what holds it, then deletes it.
//!
//! The tick rate: everything here steps in Halo ticks (`halo_ticks`: the
//! 30 Hz boundaries the engine tick crossed, from `SimClock`), so at 30 Hz
//! it runs exactly once per engine tick and under another tick (a party
//! app's 64 Hz) on the engine ticks that cross a Halo tick, 30 times a
//! second of simulated time; a press between two Halo ticks is kept for
//! the next (`HaloHands`' latches). Both sides of a prediction count the
//! same ticks from the command's time.
//!
//! Known shortcuts: a shot starts at the eye (the weapon's trigger marker,
//! moved onto the camera ray, has no model yet to come from), then the
//! trigger's first-person offset; a fired projectile's flight is the
//! server's (the shooter's client predicts the weapon, ammo, heat, the
//! grenade count and the shot's `HaloFired`; every machine shows the nine
//! guns' projectiles by flying them again from the shot with the same
//! flight rules and their own random generator, `fx::shots`, the server
//! sending others' shots (guided ones unguided; the grenades are not shown
//! yet); later updates of a projectile meet
//! characters where they
//! are now (lag compensation rewinds the firing tick's update only); the
//! hit's "shield or body" material is the target's shield above 0 before
//! the hit (the seam knows the exact share only after); dead bodies are
//! not hit; a character hit is material type 21 (cyborg) whatever its
//! game; a projectile stuck in a character keeps its offset from the
//! character's centre (no node, no turn); guidance aims at the target's
//! centre (no "body" marker); the target is picked among characters by
//! their centres (no autoaim pill); the multiplayer difficulty's
//! guidance-against-players factor is normal's 0.5 (projectiles.md open
//! question 2); the melee's 25 probes meet characters only (no breakable
//! surfaces, no vehicles), and its response damage on the attacker is not
//! applied (all zero on the disc).

use std::sync::Arc;

use avian3d::prelude::{Collider, ColliderAabb, ColliderOf, Position, Rotation, SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemParam, prelude::*};
use chimera_sim::weapon::{
    Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, Zoomed, aim_of, hitgroup_at,
    lagcomp::{LagComp, Rewound},
    projectile::ray_box,
};
use serde::{Deserialize, Serialize};

use super::{
    METERS_PER_WU, aim,
    explode::{self, Candidate},
    flight::{End, Flight, Hit, Steer, wandered},
    grenades::{self, GrenadeData, Throw, ThrowEvent},
    materials::LevelMaterials,
    params::{Gun, Projectile, effect_id, projectile_flags, weapon_flags},
    random::HaloRng,
    tick::{GunEvent, HaloGun, Inputs, TriggerMode},
};
use crate::{
    core::{
        ActionRegistry, BodyRegion, FirstTimePredicted, Health, HitShapes, Intent, MovementState, NetRole, SimClock,
        Team, Velocity,
    },
    map::interp::NetDrawn,
};

use super::super::{
    damage::{HaloHit, HaloHurt},
    vitality::{DamageEffects, EffectId, Spartan, Vitality, melee_scale},
};

/// The named actions Halo's weapons read beside fire, attack2 and reload.
pub const MELEE_ACTION: &str = "melee";
pub const ZOOM_ACTION: &str = "zoom";

/// A character's material type (cyborg).
pub const CYBORG: u8 = 21;
/// A Spartan's shield's material type (cyborg energy shield).
pub const CYBORG_SHIELD: u8 = 22;
/// The supercombine's count: more than this many needles.
pub const COMBINE_COUNT: usize = 6;
/// The guidance factor against a player's unit (the globals' "guidance vs
/// player" at normal difficulty).
pub const GUIDANCE_VS_PLAYER: f32 = 0.5;

/// A Halo weapon's numbers (its data's `halo_ce:weapon` table).
#[derive(Component, Clone, Debug)]
pub struct HaloWeapon(pub Arc<Gun>);

/// The 30 Hz ticks whose boundary this engine tick crossed (module doc):
/// 1 at Halo's own tick.
pub fn halo_ticks(clock: &SimClock) -> u32 {
    super::super::ticks::halo_ticks(clock)
}

/// A holder's own part of Halo's weapon rules: the player melee's
/// counter, the ticks airborne (the melee's lunge scale), the grenades
/// carried and the throw, the inputs seen since the last Halo tick (a
/// press between two is not lost).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HaloHands {
    /// Ticks of melee left (0: none running).
    pub melee: i32,
    /// The counter's value on the melee's hit tick.
    pub melee_hit: i32,
    pub airborne: u32,
    pub fire: bool,
    pub zoom: bool,
    pub melee_button: bool,
    pub reload: bool,
    pub grenade_button: bool,
    /// The grenade button was down on the last Halo tick (a throw starts on
    /// a press).
    pub grenade_held: bool,
    /// The rotate-grenades button was down since the last Halo tick, and
    /// on it (a change per press).
    pub switch_button: bool,
    pub switch_held: bool,
    /// The grenades and the throw.
    pub throw: Throw,
    /// The counts were given for this life.
    pub armed: bool,
}

/// One round fired by a Halo weapon (for presentation: the first-person
/// fire animation, the muzzle effect by entry, the sound, a tracer): on
/// the shooter's tick, predicted, not on a replay.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloFired {
    pub owner: Entity,
    pub weapon: Entity,
    pub id: &'static str,
    /// The trigger (0: the fire button's; 1: the plasma pistol's charged
    /// bolt).
    pub trigger: u8,
    pub misfired: bool,
    pub last_round: bool,
    /// The firing effect entry (the plasma rifle's two barrels).
    pub effect: u8,
    /// From where (metres) and each projectile's direction and tracer.
    pub origin: Vec3,
    pub projectiles: Vec<(Vec3, bool)>,
    /// The weapon's heat and battery after the shot.
    pub heat: f32,
    pub age: f32,
    /// A guided projectile's target (section 15's, where the gun's data
    /// gives it one: the needles, the charged bolt).
    pub target: Option<Entity>,
}

/// A projectile's segment of one update (server): from, to (metres), what
/// it met there (None: nothing; `Some(None)`: the level), the normal, the
/// material type met (None: none or material-less).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloImpact {
    pub owner: Entity,
    pub id: &'static str,
    pub from: Vec3,
    pub to: Vec3,
    pub hit: Option<Option<Entity>>,
    pub normal: Vec3,
    pub tracer: bool,
    pub material: Option<u8>,
}

/// A projectile's detonation (server, for presentation: its effect where
/// it went off): the projectile tag, where (metres), whether the needles'
/// super detonation, the damage effects dealt.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloDetonation {
    pub owner: Entity,
    pub tag: String,
    pub at: Vec3,
    pub supercombine: bool,
    pub effects: Vec<EffectId>,
    /// Its flight's direction then (unit, engine axes; down for one at
    /// rest): the detonation effect's forward.
    pub forward: Vec3,
}

/// A projectile stuck (server, for presentation: the needles in a body,
/// a plasma grenade's "detonation started" effect): into a character
/// (None: the level) at a point (metres).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloStuck {
    pub projectile: Entity,
    pub tag: String,
    pub target: Option<Entity>,
    pub at: Vec3,
}

/// A push from a damage event (projectiles.md 11; server): the velocity
/// change in metres per second, applied by `apply_pushes`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HaloPush {
    pub target: Entity,
    pub dv: Vec3,
}

/// What holds a projectile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holder {
    /// Free.
    None,
    /// In a thrower's hand (a grenade before its release), since this
    /// tick (`SimClock::tick`) with its power's count.
    Hand { thrower: Entity, ticks: i32 },
    /// Stuck in a character at an offset from its centre (metres), in the
    /// order needles stuck (`seq`).
    Stuck { target: Entity, offset: Vec3, seq: u64 },
}

/// A projectile in flight (server).
#[derive(Component, Clone, Debug)]
pub struct HaloProjectile {
    pub proj: Arc<Projectile>,
    pub id: &'static str,
    pub owner: Entity,
    pub flight: Flight,
    pub ignored: Option<Entity>,
    pub rng: u32,
    pub tracer: bool,
    /// The guidance's target.
    pub target: Option<Entity>,
    /// A stable integer for the wander.
    pub k: u32,
    pub holder: Holder,
    /// A needle a supercombine has consumed.
    pub consumed: bool,
    /// How an update outside `fly` (its first, or a grenade's release)
    /// ended it, for `settle_new`.
    pub ended: Option<End>,
}

/// What a projectile or a melee probe traces: the level (avian, with the
/// level's material types) and the characters' hit volumes (their
/// hitboxes, else their box), the shooter's view of them when lag
/// compensation rewound them.
#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct HaloTrace<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    shapes: Query<'w, 's, (&'static Collider, Option<&'static Position>, Option<&'static Rotation>, Option<&'static Name>)>,
    materials: Option<Res<'w, LevelMaterials>>,
    characters: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static ColliderAabb>,
            HitShapes,
            Option<&'static Intent>,
            &'static Health,
            Option<&'static Vitality>,
        ),
        With<Intent>,
    >,
    teams: Query<'w, 's, &'static Team>,
}

/// What a trace met (metres): the distance along it, the point, the
/// normal, the character with the region hit (None: the level), the
/// level's material type (None: material-less, or a character).
pub type Met = (f32, Vec3, Vec3, Option<(Entity, Option<BodyRegion>)>, Option<u8>);

impl HaloTrace<'_, '_> {
    /// The nearest thing on `from` to `to` (metres), leaving out `skip`.
    pub fn trace(&self, from: Vec3, to: Vec3, skip: &[Entity], rewound: &[Rewound]) -> Option<Met> {
        self.trace_with(from, to, skip, rewound, true)
    }

    /// `trace`, against characters alone when `level` is false.
    pub fn trace_with(&self, from: Vec3, to: Vec3, skip: &[Entity], rewound: &[Rewound], level: bool) -> Option<Met> {
        let (dir, len) = Dir3::new_and_length(to - from).ok()?;
        let mut best: Option<Met> = if level { self.level(from, *dir, len) } else { None };
        let limit = |b: &Option<Met>| b.as_ref().map_or(len, |b| b.0);
        for (e, t, aabb, shapes, intent, health, _) in &self.characters {
            if skip.contains(&e) || health.current <= 0.0 || rewound.iter().any(|r| r.entity == e) {
                continue;
            }
            let lim = limit(&best);
            if let Some(boxes) = shapes.get() {
                let feet = aabb.map_or(t.translation, |b| t.translation.with_y(b.min.y));
                let turn = intent.map_or(Quat::IDENTITY, |i| i.yaw_rotation()).inverse();
                if let Some((i, enter, _)) = boxes.nearest(turn * (from - feet), turn * *dir, lim) {
                    best = Some((enter, from + *dir * enter, -*dir, Some((e, Some(boxes.0[i].region))), None));
                }
            } else if let Some(b) = aabb
                && let Some(enter) = ray_box(b.min, b.max, from, *dir, lim)
            {
                let p = from + *dir * enter;
                best = Some((enter, p, -*dir, Some((e, Some(BodyRegion::from_hitgroup(hitgroup_at(b, p))))), None));
            }
        }
        for r in rewound.iter().filter(|r| !skip.contains(&r.entity)) {
            let lim = limit(&best);
            if let Some(boxes) = r.hitboxes.as_ref().map(|h| h.get()) {
                let (feet, turn) = r.frame();
                if let Some((i, enter, _)) = boxes.nearest(turn * (from - feet), turn * *dir, lim) {
                    best = Some((enter, from + *dir * enter, -*dir, Some((r.entity, Some(boxes.0[i].region))), None));
                }
            } else if let Some(enter) = r.box_entry(from, *dir, lim, 0.0) {
                let p = from + *dir * enter;
                best = Some((enter, p, -*dir, Some((r.entity, Some(BodyRegion::from_hitgroup(hitgroup_at(&r.aabb(), p))))), None));
            }
        }
        best
    }

    /// Whether the level has Halo's material table (else its surfaces
    /// are stone by default, `materials::NO_TABLE`).
    pub fn has_materials(&self) -> bool {
        self.materials.is_some()
    }

    /// The level alone: where the ray meets it and its material type.
    fn level(&self, from: Vec3, dir: Vec3, len: f32) -> Option<Met> {
        let dir3 = Dir3::new(dir).ok()?;
        let all: Vec<Entity> = self.characters.iter().map(|c| c.0).collect();
        let filter = SpatialQueryFilter::from_excluded_entities(all).with_mask(crate::core::SOLID_LAYERS);
        let colliders = &self.colliders;
        let chars = &self.characters;
        let h = self.spatial.cast_ray_predicate(from, dir3, len, true, &filter, &|c| {
            let body = colliders.get(c).map_or(c, |o| o.body);
            !chars.contains(body)
        })?;
        let material = match &self.materials {
            Some(m) => m.at(self.shapes.get(h.entity).ok(), from, dir, h.distance),
            None => Some(super::materials::NO_TABLE),
        };
        // The point a hair before the surface (1e-5 m back along the ray),
        // so the next segment starts outside it.
        let d = (h.distance - 1e-5).max(0.0);
        Some((d, from + dir * d, h.normal, None, material))
    }

    /// Whether the line `from` to `to` (metres) is clear of the level.
    pub fn clear(&self, from: Vec3, to: Vec3) -> bool {
        match Dir3::new_and_length(to - from) {
            Ok((dir, len)) => self.level(from, *dir, len).is_none(),
            Err(_) => true,
        }
    }

    /// Whether the shield takes a hit on `target` (a Halo character with
    /// shield left), else the body (projectiles.md 6, step 1).
    pub fn shielded(&self, target: Entity) -> bool {
        self.characters.get(target).ok().and_then(|c| c.6).is_some_and(|v| v.shield > 0.0)
    }

    /// A living character's centre (metres).
    pub fn centre(&self, e: Entity) -> Option<Vec3> {
        self.characters.get(e).ok().filter(|c| c.5.current > 0.0).map(|c| c.1.translation)
    }

    /// A character's centre, alive or dead.
    pub fn any_centre(&self, e: Entity) -> Option<Vec3> {
        self.characters.get(e).ok().map(|c| c.1.translation)
    }

    /// The living characters: entity, centre (metres), camouflage.
    pub fn living(&self) -> impl Iterator<Item = (Entity, Vec3, f32)> + '_ {
        self.characters
            .iter()
            .filter(|c| c.5.current > 0.0)
            .map(|c| (c.0, c.1.translation, c.6.map_or(0.0, |v| v.camo)))
    }

    fn team(&self, e: Entity) -> u8 {
        self.teams.get(e).map_or(0, |t| t.0)
    }

    /// Section 15's target for a shot (weapons.md: the autoaim and
    /// magnetism levels by distance and angle from the camera ray, sorted
    /// by them, then distance, then angle; the first in sight): living
    /// enemies not fully camouflaged. The target, its aim point (metres)
    /// and its autoaim level.
    pub fn target(&self, owner: Entity, eye: Vec3, aim: Vec3, a: &super::aim::Assist) -> Option<(Entity, Vec3, f32)> {
        use super::aim::level;
        let (range, angle) = (a.autoaim_range.max(a.magnetism_range), a.autoaim_angle.max(a.magnetism_angle));
        let me = self.team(owner);
        let mut found: Vec<(f32, f32, f32, f32, Entity, Vec3)> = self
            .living()
            .filter(|(e, _, camo)| *e != owner && *camo < 1.0 && (me == 0 || self.team(*e) != me))
            .filter_map(|(e, c, _)| {
                let to = c - eye;
                let d = to.length() / METERS_PER_WU;
                let ang = aim.angle_between(to);
                if d > range || ang > angle {
                    return None;
                }
                let auto = level(d, a.autoaim_range) * level(ang, a.autoaim_angle);
                let magnet = level(d, a.magnetism_range) * level(ang, a.magnetism_angle);
                (auto > 0.0 || magnet > 0.0).then_some((auto, magnet, d, ang, e, c))
            })
            .collect();
        found.sort_by(|x, y| y.0.total_cmp(&x.0).then(y.1.total_cmp(&x.1)).then(x.2.total_cmp(&y.2)).then(x.3.total_cmp(&y.3)));
        found.into_iter().find(|f| self.clear(eye, f.5)).map(|f| (f.4, f.5, f.0))
    }

    /// Where the camera at `eye` looking along `aim` (unit) looks: the
    /// level's surface within `aim::LOOK_RANGE`, else that far (metres).
    pub fn look_at(&self, eye: Vec3, aim: Vec3) -> Vec3 {
        let len = super::aim::LOOK_RANGE * METERS_PER_WU;
        self.level(eye, aim, len).map_or(eye + aim * len, |m| m.1)
    }
}

/// What a fired round's first update and later ones need on the server.
#[derive(SystemParam)]
pub struct Hits<'w> {
    hits: MessageWriter<'w, HaloHit>,
    impacts: MessageWriter<'w, HaloImpact>,
    stuck: MessageWriter<'w, HaloStuck>,
    effects: Res<'w, DamageEffects>,
}

/// One update of a free or stuck projectile on the server: its flight,
/// its direct hits through the seam and its segments for presentation.
/// Its end, if it ended.
fn advance(e: Entity, p: &mut HaloProjectile, trace: &HaloTrace, rewound: &[Rewound], out: &mut Hits, tick: u64) -> Option<End> {
    let proj = p.proj.clone();
    let owner = p.owner;
    let effect = proj.impact_effect();
    // Stuck: it moves with what holds it; a holder gone detonates it.
    if let Holder::Stuck { target, offset, .. } = p.holder {
        match trace.any_centre(target) {
            Some(c) => p.flight.position = (c + offset) / METERS_PER_WU,
            None => {
                p.holder = Holder::None;
                p.flight.arming = 1.0;
                p.flight.timer = 1.0;
                p.flight.request = super::flight::Request::Detonate;
            }
        }
    }
    let steer = p.target.and_then(|t| trace.centre(t)).filter(|_| proj.guided_angular_velocity > 0.0).map(|c| {
        let at = c / METERS_PER_WU;
        let e = (at - p.flight.position).length();
        let player = true;
        let omega = proj.guided_angular_velocity / 30.0 * if player { GUIDANCE_VS_PLAYER } else { 1.0 };
        Steer { target: wandered(at, e, tick, p.k), omega }
    });
    let mut rng = HaloRng(p.rng);
    // The damage's draws, apart from the flight's.
    let mut draws = HaloRng(p.rng.rotate_left(16) ^ 0x2545_F491);
    let mut hits = Vec::new();
    type Struck = (Entity, Option<BodyRegion>);
    let radius = proj.collision_radius;
    let moved = {
        let mut tracer = |a: Vec3, b: Vec3, ignored: Option<Struck>| -> Option<Hit<Struck>> {
            let skip: Vec<Entity> = std::iter::once(owner).chain(ignored.map(|i| i.0)).collect();
            let (am, bm) = (a * METERS_PER_WU, b * METERS_PER_WU);
            let mut met = trace.trace(am, bm, &skip, rewound);
            // A projectile with a collision radius widens against objects
            // only, sideways, when the line met nothing (behaviour 5).
            if met.is_none() && radius >= 0.0001 {
                let l = Vec3::Y.cross(b - a).try_normalize().unwrap_or(Vec3::NEG_X) * radius * METERS_PER_WU;
                met = [l, -l].into_iter().find_map(|o| trace.trace_with(am + o, bm + o, &skip, rewound, false));
            }
            let (_, point, normal, what, material) = met?;
            let len = (bm - am).length();
            let t = if len > 0.0 { ((point - am).length() / len).clamp(0.0, 1.0) } else { 0.0 };
            let point = a + (b - a) * t;
            Some(Hit {
                t,
                point,
                normal,
                object: what,
                material: if what.is_some() { Some(CYBORG) } else { material },
                unit: what.is_some(),
            })
        };
        let mut struck = |h: &Hit<Struck>, v: Vec3, s: f32| -> Option<u8> {
            let (target, region) = h.object?;
            let effect = effect?;
            let fx = out.effects.get(effect)?;
            let u = draws.real();
            let amount = fx.amount(s, u);
            if amount <= 0.0 {
                return None;
            }
            let shield = trace.shielded(target);
            hits.push((target, region, h.point, v, s, amount));
            Some(if shield { CYBORG_SHIELD } else { CYBORG })
        };
        let mut ignored = p.ignored.map(|e| (e, None));
        let m = p.flight.update(&proj, &mut ignored, &mut rng, steer, &mut tracer, &mut struck);
        p.ignored = ignored.map(|i| i.0);
        m
    };
    p.rng = rng.0 ^ draws.0.rotate_left(7);
    for (target, region, point, v, s, amount) in hits {
        let Some(effect) = effect else { continue };
        out.hits.write(
            HaloHit::direct(target, effect, amount, s, region, point * METERS_PER_WU, v.normalize_or_zero(), Some(owner)).weapon(p.id),
        );
    }
    for (from, to, hit) in &moved.segments {
        if let Some(h) = hit {
            debug!("{} impact at {} on {}", p.id, *to * METERS_PER_WU, h.object.map_or("the level".to_string(), |o| format!("{:?}", o.0)));
        }
        out.impacts.write(HaloImpact {
            owner,
            id: p.id,
            from: *from * METERS_PER_WU,
            to: *to * METERS_PER_WU,
            hit: hit.map(|h| h.object.map(|o| o.0)),
            normal: hit.map_or(Vec3::ZERO, |h| h.normal),
            tracer: p.tracer,
            material: hit.and_then(|h| h.material),
        });
    }
    if let Some(h) = moved.stuck {
        let at = h.point * METERS_PER_WU;
        if let Some((target, _)) = h.object
            && let Some(c) = trace.any_centre(target)
        {
            p.holder = Holder::Stuck { target, offset: at - c, seq: tick * 1024 + (e.index_u32() as u64 & 1023) };
        }
        out.stuck.write(HaloStuck { projectile: e, tag: proj.tag.clone(), target: h.object.map(|o| o.0), at });
    }
    moved.end
}

/// The projectiles already out, one update per Halo tick (server; before
/// the weapons, which give the new ones their first update), then the
/// detonations.
#[allow(clippy::too_many_arguments)]
pub fn fly(
    mut projectiles: Query<(Entity, &mut HaloProjectile)>,
    trace: HaloTrace,
    mut out: Hits,
    mut blasts: Blasts,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    for step in 0..n {
        let tick = clock.tick + step as u64;
        let mut ends: Vec<(Entity, End)> = Vec::new();
        let mut stuck_now: Vec<(Entity, Entity, String)> = Vec::new();
        for (e, mut p) in &mut projectiles {
            if ends.iter().any(|x| x.0 == e) {
                continue;
            }
            if let Holder::Hand { .. } = p.holder {
                // In the hand: the timers alone (the release moves it).
                hold(&mut p);
                if p.flight.request == super::flight::Request::Disappear {
                    ends.push((e, End::Disappear));
                }
                continue;
            }
            let was_stuck = matches!(p.holder, Holder::Stuck { .. });
            if let Some(end) = advance(e, &mut p, &trace, &[], &mut out, tick) {
                ends.push((e, end));
            }
            if !was_stuck
                && let Holder::Stuck { target, .. } = p.holder
            {
                stuck_now.push((e, target, p.proj.tag.clone()));
            }
        }
        // A needle sticking resets the unconsumed needles of its tag
        // already in that body (behaviour 13).
        for (new, target, tag) in &stuck_now {
            for (e, mut p) in &mut projectiles {
                if e != *new
                    && !p.consumed
                    && p.proj.has(projectile_flags::HAS_SUPER_COMBINING_EXPLOSION)
                    && p.proj.tag == *tag
                    && matches!(p.holder, Holder::Stuck { target: t, .. } if t == *target)
                {
                    p.flight.timer = 0.0;
                    p.flight.arming = 0.0;
                }
            }
        }
        for (e, end) in ends {
            if end == End::Detonate {
                detonate(e, &mut projectiles, &trace, &mut out, &mut blasts);
            }
            commands.entity(e).despawn();
        }
    }
}

/// The projectiles an update outside `fly` ended (their first update on
/// the firing tick, a grenade's on its release): detonated or deleted on
/// that tick (server, after the weapons).
pub fn settle_new(
    mut projectiles: Query<(Entity, &mut HaloProjectile)>,
    trace: HaloTrace,
    mut out: Hits,
    mut blasts: Blasts,
    mut commands: Commands,
) {
    let ended: Vec<(Entity, End)> = projectiles.iter_mut().filter_map(|(e, mut p)| p.ended.take().map(|x| (e, x))).collect();
    for (e, end) in ended {
        if end == End::Detonate {
            detonate(e, &mut projectiles, &trace, &mut out, &mut blasts);
        }
        commands.entity(e).despawn();
    }
}

/// A held grenade's tick: its timers run, it does not move.
fn hold(p: &mut HaloProjectile) {
    let f = &mut p.flight;
    f.updates += 1;
    f.arming += f.arming_step;
    if let Holder::Hand { thrower, ticks } = p.holder {
        p.holder = Holder::Hand { thrower, ticks: ticks + 1 };
    }
}

/// What a detonation needs beside the trace: the explosions' numbers and
/// the presentation's and the pushes' messages.
#[derive(SystemParam)]
pub struct Blasts<'w> {
    grenades: Res<'w, GrenadeData>,
    detonations: MessageWriter<'w, HaloDetonation>,
    pushes: MessageWriter<'w, HaloPush>,
}

/// A projectile's detonation (behaviour 9): the supercombine check, the
/// detonation effect's area damage and pushes, a stuck one's attached
/// damage.
fn detonate(e: Entity, projectiles: &mut Query<(Entity, &mut HaloProjectile)>, trace: &HaloTrace, out: &mut Hits, blasts: &mut Blasts) {
    let Ok((_, p)) = projectiles.get(e) else { return };
    let p = p.clone();
    let mut rng = HaloRng(p.rng ^ 0x5EED);
    let mut at = p.flight.position;
    let mut effects = &p.proj.detonation;
    let mut attached = p.proj.attached_damage.as_deref().and_then(effect_id);
    let mut supercombine = false;
    if let Holder::Stuck { target, .. } = p.holder
        && p.proj.has(projectile_flags::HAS_SUPER_COMBINING_EXPLOSION)
        && trace.any_centre(target).is_some()
    {
        // The needles of this tag stuck in the target, unconsumed, in the
        // order they stuck.
        let mut list: Vec<(u64, Entity)> = projectiles
            .iter()
            .filter(|(_, q)| !q.consumed && q.proj.tag == p.proj.tag)
            .filter_map(|(qe, q)| match q.holder {
                Holder::Stuck { target: t, seq, .. } if t == target => Some((seq, qe)),
                _ => None,
            })
            .collect();
        if list.len() > COMBINE_COUNT {
            list.sort();
            let mut count = list.len();
            for (_, qe) in &list {
                if *qe != e
                    && let Ok((_, mut q)) = projectiles.get_mut(*qe)
                {
                    if count <= COMBINE_COUNT {
                        q.consumed = true;
                        q.flight.timer *= rng.real();
                        q.flight.arming *= rng.real();
                    } else {
                        q.flight.timer = 0.0;
                        q.flight.arming = 0.0;
                    }
                }
                count -= 1;
            }
            supercombine = true;
            effects = &p.proj.super_detonation;
            attached = None;
            // Detached to the target's feet.
            if let Ok((_, t, aabb, ..)) = trace.characters.get(target) {
                let feet = aabb.map_or(t.translation, |b| t.translation.with_y(b.min.y));
                at = feet / METERS_PER_WU;
            }
        }
    }
    let x = blasts.grenades.explosions;
    let em = at * METERS_PER_WU;
    // The attached damage on what holds it (no falloff, the indirect
    // material).
    if let (Holder::Stuck { target, .. }, Some(id)) = (p.holder, attached)
        && let Some(fx) = out.effects.get(id)
    {
        let dir = p.flight.velocity.normalize_or(Vec3::NEG_Y);
        out.hits.write(HaloHit::direct(target, id, fx.amount(1.0, rng.real()), 1.0, None, em, dir, Some(p.owner)).weapon(p.id));
    }
    let ids: Vec<EffectId> = effects.iter().filter_map(|n| effect_id(n)).collect();
    let r = x.biped_bounding_radius;
    let candidates: Vec<Candidate<Entity>> = trace
        .living()
        .map(|(c, centre, _)| Candidate { object: c, centre: centre / METERS_PER_WU, radius: r, unit: true, owner: c == p.owner })
        .collect();
    let grenades: Vec<(Entity, Vec3)> = projectiles
        .iter()
        .filter(|(q, h)| *q != e && matches!(h.holder, Holder::None) && (h.id == FRAG_GRENADE || h.id == PLASMA_GRENADE))
        .map(|(q, h)| (q, h.flight.position))
        .collect();
    for id in &ids {
        let Some(fx) = out.effects.get(*id) else { continue };
        let mut clear = |a: Vec3, b: Vec3| trace.clear(a * METERS_PER_WU, b * METERS_PER_WU);
        let mut draw = || rng.real();
        for h in explode::area(fx, at, &candidates, &mut clear, &mut draw) {
            debug!("{} {}: {:?} at scale {} takes {} points", p.id, id.0, h.object, h.scale, h.amount);
            out.hits.write(HaloHit::area(h.object, *id, h.amount, h.scale, em, h.dir, Some(p.owner)).weapon(p.id));
            if h.amount > 0.0
                && let Some(dv) = explode::push(&x, fx, h.dir, x.biped_acceleration_scale, true)
            {
                debug!("{} pushes {:?} by {dv} WU a tick", p.id, h.object);
                blasts.pushes.write(HaloPush { target: h.object, dv: dv * METERS_PER_WU * 30.0 });
            }
        }
        // Grenades lying or flying near: pushed, never set off.
        let [_, r1] = fx.radius;
        for (q, pos) in &grenades {
            let w = *pos - at;
            if w.length() < r1
                && fx.lower + fx.upper[1] > 0.0
                && trace.clear(em, *pos * METERS_PER_WU)
                && let Some(dv) = explode::push(&x, fx, w.normalize_or_zero(), x.grenade_acceleration_scale, false)
                && let Ok((_, mut g)) = projectiles.get_mut(*q)
            {
                g.flight.push(dv);
            }
        }
    }
    debug!("{} detonated at {em}{}", p.id, if supercombine { " (supercombine)" } else { "" });
    let forward = if supercombine { Vec3::NEG_Y } else { p.flight.velocity.normalize_or(Vec3::NEG_Y) };
    blasts.detonations.write(HaloDetonation { owner: p.owner, tag: p.proj.tag.clone(), at: em, supercombine, effects: ids, forward });
}

/// The tick's pushes on characters (server): Halo's movement through its
/// impulse seam (`movement::push`), any other body by its velocity.
pub fn apply_pushes(
    mut pushes: MessageReader<HaloPush>,
    mut q: Query<(&mut Velocity, Option<&mut super::super::movement::HaloMovement>)>,
) {
    for p in pushes.read() {
        if let Ok((mut v, me)) = q.get_mut(p.target) {
            super::super::movement::push(me.map(|m| m.into_inner()), &mut v, p.dv);
        }
    }
}

/// Every Halo holder's tick (module doc): in the predicted weapons stage.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn run(
    mut holders: Query<
        (Entity, &Intent, &Inventory, &Transform, &MovementState, Option<&Velocity>, &Health, &mut HaloHands),
        Without<NetDrawn>,
    >,
    mut guns: Query<(&Weapon, &HaloWeapon, &mut HaloGun, Option<&mut Magazine>)>,
    mut held: Query<(Entity, &mut HaloProjectile)>,
    trace: HaloTrace,
    mut lag: LagComp,
    mut out: Hits,
    mut fired: MessageWriter<HaloFired>,
    mut events: MessageWriter<WeaponEvent>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<NetRole>>,
    actions: Option<Res<ActionRegistry>>,
    spartan: Option<Res<Spartan>>,
    grenades: Res<GrenadeData>,
    mut most: Local<u32>,
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    let server = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    let bit = |name: &str| actions.as_deref().map_or(0, |a| a.bit(name));
    let (melee_bit, zoom_bit, grenade_bit) = (bit(MELEE_ACTION), bit(ZOOM_ACTION), bit(grenades::GRENADE_ACTION));
    let switch_bit = bit(grenades::SWITCH_ACTION);
    *most = (*most).max(holders.iter().count() as u32);
    let players = *most;
    for (owner, intent, inv, at, state, vel, health, mut hands) in &mut holders {
        if !inv.weapons.iter().any(|w| guns.contains(*w)) {
            continue;
        }
        let alive = health.current > 0.0;
        // A new life's grenades.
        if !alive {
            hands.armed = false;
        } else if !hands.armed {
            hands.armed = true;
            hands.throw = Throw::new(grenades.start(grenades.cap(players)));
        }
        // The inputs seen since the last Halo tick.
        hands.fire |= intent.fire;
        hands.zoom |= intent.secondary || intent.actions & zoom_bit != 0;
        hands.melee_button |= intent.actions & melee_bit != 0;
        hands.reload |= intent.reload;
        hands.grenade_button |= grenade_bit != 0 && intent.actions & grenade_bit != 0;
        hands.switch_button |= switch_bit != 0 && intent.actions & switch_bit != 0;
        if n == 0 {
            continue;
        }
        let eye = at.translation + state.eye_offset;
        let aim = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        for _ in 0..n {
            // 2. The unit: a switch puts the old weapon away and readies
            // the new one at once.
            for &w in &inv.weapons {
                let Ok((_, hw, mut gun, _)) = guns.get_mut(w) else { continue };
                let held = inv.active == Some(w) && alive;
                if held != gun.in_hand {
                    let ev = if held { gun.ready(&hw.0) } else { gun.put_away(&hw.0) };
                    if let Some(GunEvent::RoundsIn { shell }) = ev {
                        event(&mut events, &first, owner, w, if shell { WeaponEventKind::ShellInserting } else { WeaponEventKind::Reloaded });
                    }
                }
            }
            // The grenade throw's state.
            let busy = hands.melee > 0 || hands.throw.busy();
            let press = hands.grenade_button && !hands.grenade_held;
            // The grenade types (weapons.md 14): the input layer, a press
            // one step.
            if alive {
                let press = hands.switch_button && !hands.switch_held;
                hands.throw.input(press);
            }
            hands.switch_held = hands.switch_button;
            hands.grenade_held = hands.grenade_button;
            let allowed = alive
                && inv
                    .active
                    .and_then(|a| guns.get(a).ok())
                    .is_none_or(|(_, hw, ..)| !hw.0.has(weapon_flags::PREVENTS_GRENADES));
            let mut throw = hands.throw;
            for ev in throw.step(&grenades.throw, press, allowed) {
                match ev {
                    ThrowEvent::Started => {
                        hands.melee = 0;
                        if let Some(a) = inv.active
                            && let Ok((_, hw, mut gun, _)) = guns.get_mut(a)
                        {
                            if let Some(GunEvent::RoundsIn { .. }) = gun.settle_reload(&hw.0) {
                                event(&mut events, &first, owner, a, WeaponEventKind::Reloaded);
                            }
                            if gun.zoom >= 0 {
                                gun.zoom = -1;
                                event(&mut events, &first, owner, a, WeaponEventKind::Action("zoom_out"));
                            }
                        }
                        if let Some(a) = inv.active {
                            event(&mut events, &first, owner, a, WeaponEventKind::Action("throw_grenade"));
                        }
                    }
                    ThrowEvent::Created(kind) => {
                        if server && first.0 {
                            let proj = Arc::new(grenades.projectile(kind).clone());
                            let flight = Flight::new(&proj, eye / METERS_PER_WU, aim, 0.0);
                            commands.spawn((
                                HaloProjectile {
                                    proj,
                                    id: if kind == grenades::PLASMA { PLASMA_GRENADE } else { FRAG_GRENADE },
                                    owner,
                                    flight,
                                    ignored: None,
                                    rng: (clock.tick as u32).wrapping_mul(0x9E37_79B9) ^ owner.index_u32(),
                                    tracer: true,
                                    target: None,
                                    k: owner.index_u32(),
                                    holder: Holder::Hand { thrower: owner, ticks: 1 },
                                    consumed: false,
                                    ended: None,
                                },
                                Name::new("halo_ce grenade"),
                            ));
                        }
                    }
                    ThrowEvent::Released(_) => {
                        if server && first.0 {
                            let (pos, v) = grenades::release(&grenades.throw, eye / METERS_PER_WU, aim);
                            for (e, mut p) in &mut held {
                                if matches!(p.holder, Holder::Hand { thrower, .. } if thrower == owner) {
                                    p.holder = Holder::None;
                                    p.flight.position = pos;
                                    p.flight.velocity = v;
                                    p.ignored = Some(owner);
                                    // Its update of this tick, moving (an end
                                    // is `settle_new`'s, after the weapons).
                                    p.ended = advance(e, &mut p, &trace, &[], &mut out, clock.tick);
                                }
                            }
                        }
                    }
                    ThrowEvent::Ended => {}
                }
            }
            hands.throw = throw;
            // The unit's grenade type follows the desired one when not busy.
            if alive {
                let occupied = hands.melee > 0 || hands.throw.busy();
                hands.throw.update(occupied);
            }
            // 3. The player melee.
            if state.on_ground {
                hands.airborne = 0;
            } else {
                hands.airborne += 1;
            }
            if alive {
                melee(
                    owner, &mut hands, inv, &mut guns, eye, aim, vel.map_or(Vec3::ZERO, |v| v.0), &trace, &mut lag, &mut out,
                    &mut events, &first, server, spartan.as_deref(),
                );
            }
            // 4. The weapons: the one in hand with the inputs, the others
            // with none.
            for &w in &inv.weapons {
                let Ok((weapon, hw, mut gun, mag)) = guns.get_mut(w) else { continue };
                let held = inv.active == Some(w) && alive;
                if let Some(m) = mag.as_deref() {
                    gun.loaded = m.clip as i16;
                    gun.reserve = m.reserve as i16;
                }
                // The aim assist (weapons.md 15, `aim`): while the weapon
                // in hand may fire, its autoaim target (when the switch is
                // on) and the shot bent toward it and the look point.
                let may_fire = held && (hands.fire || matches!(gun.trigger, TriggerMode::Charging | TriggerMode::Charged));
                let assist = may_fire.then(|| aim::assist(&hw.0, gun.zoom)).flatten();
                let picked = assist
                    .filter(|_| hw.0.aim_assist.enabled)
                    .and_then(|a| trace.target(owner, eye, aim, &a));
                let shot_aim = match assist {
                    Some(a) => {
                        let [x, y, z] = hw.0.trigger.first_person_offset;
                        let mut origin = eye / METERS_PER_WU;
                        if !hw.0.trigger.has(super::params::trigger_flags::PROJECTILE_VECTOR_CANNOT_BE_ADJUSTED) {
                            let left = Vec3::Y.cross(aim).try_normalize().unwrap_or(Vec3::Y);
                            origin += aim * x + left * y + aim.cross(left) * z;
                        }
                        let look = trace.look_at(eye, aim) / METERS_PER_WU;
                        aim::bend(aim, origin, look, picked.map(|(_, at, l)| (at / METERS_PER_WU, l)), a.deviation)
                    }
                    None => aim,
                };
                let inputs = Inputs {
                    fire: held && hands.fire,
                    reload: held && hands.reload,
                    zoom: held && hands.zoom,
                    busy,
                    held,
                    switching: false,
                    aim: shot_aim,
                };
                let evs = gun.tick(&hw.0, &inputs);
                if let Some(mut m) = mag {
                    m.clip = gun.loaded.max(0) as u32;
                    m.reserve = gun.reserve.max(0) as u32;
                }
                for ev in evs {
                    match ev {
                        GunEvent::Fired { trigger, misfired, last_round, effect, projectiles } => {
                            let kind = if misfired { "misfire" } else if last_round { "fire_last" } else { "fire" };
                            if first.0 {
                                debug!("{} fired trigger {trigger} on tick {}: {} loaded, {} reserve, heat {}", weapon.id, clock.tick, gun.loaded, gun.reserve, gun.heat);
                            }
                            event(&mut events, &first, owner, w, WeaponEventKind::Action(if trigger == 1 { "fire_charged" } else { kind }));
                            // A guided projectile's target, the same on
                            // both sides of the prediction.
                            let guided = match (&hw.0.secondary, trigger) {
                                (Some(s), 1) => s.projectile.guided_angular_velocity,
                                _ => hw.0.projectile.guided_angular_velocity,
                            } > 0.0;
                            // The autoaim's target (projectiles.md 12: no
                            // separate test for guided projectiles).
                            let target = picked.filter(|_| first.0 && guided).map(|(e, ..)| e);
                            if first.0 {
                                fired.write(HaloFired {
                                    owner,
                                    weapon: w,
                                    id: weapon.id,
                                    trigger,
                                    misfired,
                                    last_round,
                                    effect,
                                    origin: eye,
                                    projectiles: projectiles.clone(),
                                    heat: gun.heat,
                                    age: gun.age,
                                    target,
                                });
                            }
                            if server && first.0 && !projectiles.is_empty() {
                                // Created and given their first update now
                                // (projectiles.md 4), against the others
                                // where the shooter saw them.
                                let g = &hw.0;
                                let (t, proj) = match (&g.secondary, trigger) {
                                    (Some(s), 1) => (&s.trigger, Arc::new(s.projectile.clone())),
                                    _ => (&g.trigger, Arc::new(g.projectile.clone())),
                                };
                                let rewound = lag.rewind(owner, true, None);
                                let inherited = vel.map_or(0.0, |v| v.0.dot(aim)) / METERS_PER_WU / 30.0;
                                // The first-person offset (forward, left,
                                // up), unless the vector cannot be adjusted.
                                let mut origin = eye / METERS_PER_WU;
                                if !t.has(super::params::trigger_flags::PROJECTILE_VECTOR_CANNOT_BE_ADJUSTED) {
                                    let left = Vec3::Y.cross(aim).try_normalize().unwrap_or(Vec3::Y);
                                    let up = aim.cross(left);
                                    let [x, y, z] = t.first_person_offset;
                                    origin += aim * x + left * y + up * z;
                                }
                                for (i, (d, tracer)) in projectiles.into_iter().enumerate() {
                                    let mut rng = HaloRng(gun.rng ^ (i as u32).wrapping_mul(0x9E37_79B9));
                                    let u = rng.real();
                                    let mut p = HaloProjectile {
                                        proj: proj.clone(),
                                        id: weapon.id,
                                        owner,
                                        flight: Flight::with_draw(&proj, origin, d, inherited, u),
                                        ignored: None,
                                        rng: rng.0,
                                        tracer,
                                        target,
                                        k: (clock.tick as u32).wrapping_add(i as u32),
                                        holder: Holder::None,
                                        consumed: false,
                                        ended: None,
                                    };
                                    let spawned = commands.spawn(Name::new("halo_ce projectile")).id();
                                    // Its first update; an end on it is
                                    // `settle_new`'s, after the weapons.
                                    p.ended = advance(spawned, &mut p, &trace, &rewound, &mut out, clock.tick);
                                    commands.entity(spawned).insert(p);
                                }
                            }
                        }
                        GunEvent::Empty => event(&mut events, &first, owner, w, WeaponEventKind::DryFire),
                        GunEvent::ReloadStarted { shell, first: f } => {
                            let kind = if shell && !f { WeaponEventKind::ShellInserting } else { WeaponEventKind::ReloadStarted };
                            event(&mut events, &first, owner, w, kind);
                        }
                        GunEvent::RoundsIn { shell } => {
                            let kind = if shell { WeaponEventKind::Action("shell_in") } else { WeaponEventKind::Reloaded };
                            event(&mut events, &first, owner, w, kind);
                        }
                        GunEvent::Chambering => event(&mut events, &first, owner, w, WeaponEventKind::Action("chamber")),
                        GunEvent::Overheated => event(&mut events, &first, owner, w, WeaponEventKind::Action("overheated")),
                        GunEvent::OverheatCleared => {
                            event(&mut events, &first, owner, w, WeaponEventKind::Action("overheat_cleared"))
                        }
                        GunEvent::Zoom(level) => {
                            let kind = if level >= 0 { "zoom_in" } else { "zoom_out" };
                            event(&mut events, &first, owner, w, WeaponEventKind::Action(kind));
                        }
                        GunEvent::Charging => event(&mut events, &first, owner, w, WeaponEventKind::Action("charging")),
                        GunEvent::Charged => event(&mut events, &first, owner, w, WeaponEventKind::Action("charged")),
                        GunEvent::Detonated => event(&mut events, &first, owner, w, WeaponEventKind::Action("detonated")),
                    }
                }
            }
        }
        hands.fire = false;
        hands.zoom = false;
        hands.melee_button = false;
        hands.reload = false;
        hands.grenade_button = false;
        hands.switch_button = false;
        // The zoom as the camera's field of view (weapons.md 12): the nominal angle over the
        // magnification, as the renderer turns it (`view::camera_fov`).
        let zoomed = inv
            .active
            .and_then(|a| guns.get(a).ok())
            .filter(|(.., g, _)| g.zoom >= 0)
            .map(|(_, hw, g, _)| Zoomed {
                fov: crate::game::view::camera_fov((hw.0.camera_field_of_view / hw.0.magnification(g.zoom)).to_degrees()),
                scope: false,
                sensitivity: Some(1.0 / hw.0.magnification(g.zoom)),
            });
        if inv.active.is_some_and(|a| guns.contains(a)) {
            match zoomed {
                Some(z) => commands.entity(owner).insert(z),
                None => commands.entity(owner).remove::<Zoomed>(),
            };
        }
    }
}

/// The frag grenade's and the plasma grenade's ids (kill notices).
pub const FRAG_GRENADE: &str = "halo_ce:frag_grenade";
pub const PLASMA_GRENADE: &str = "halo_ce:plasma_grenade";

/// A held grenade whose thrower died with it in the hand (a cut throw:
/// projectiles.md 15 step 3) is released where the thrower was, at the
/// power its time in the hand gives; one whose thrower is gone is
/// deleted (server).
pub fn cut_throws(
    mut projectiles: Query<&mut HaloProjectile>,
    throwers: Query<(&Transform, &MovementState, &Intent, &Health)>,
    grenades: Res<GrenadeData>,
    clock: Res<SimClock>,
) {
    if halo_ticks(&clock) == 0 {
        return;
    }
    let d = &grenades.throw;
    for mut p in &mut projectiles {
        let Holder::Hand { thrower, ticks } = p.holder else { continue };
        match throwers.get(thrower) {
            Ok((t, s, i, h)) if h.current <= 0.0 => {
                let aim = aim_of(i.yaw, i.pitch) * Vec3::NEG_Z;
                let (pos, full) = grenades::release(d, (t.translation + s.eye_offset) / METERS_PER_WU, aim);
                let power = ticks as f32 / (d.key_frame - d.created_frame + 1) as f32;
                let mut rng = HaloRng(p.rng);
                let v = grenades::cut(d, full, aim, power, rng.real());
                p.rng = rng.0;
                p.holder = Holder::None;
                p.flight.position = pos;
                p.flight.velocity = v;
                p.ignored = Some(thrower);
            }
            Ok(_) => {}
            Err(_) => {
                p.holder = Holder::None;
                p.flight.request = super::flight::Request::Disappear;
            }
        }
    }
}

/// A weapon event (a replay marked as one).
fn event(events: &mut MessageWriter<WeaponEvent>, first: &FirstTimePredicted, owner: Entity, weapon: Entity, kind: WeaponEventKind) {
    events.write(WeaponEvent { owner, weapon, kind, replay: !first.0 });
}

/// A player's melee, one tick (weapons.md 13): start (settling a reload)
/// or count down, the hit on its tick.
#[allow(clippy::too_many_arguments)]
fn melee(
    owner: Entity,
    hands: &mut HaloHands,
    inv: &Inventory,
    guns: &mut Query<(&Weapon, &HaloWeapon, &mut HaloGun, Option<&mut Magazine>)>,
    eye: Vec3,
    aim: Vec3,
    velocity: Vec3,
    trace: &HaloTrace,
    lag: &mut LagComp,
    out: &mut Hits,
    events: &mut MessageWriter<WeaponEvent>,
    first: &FirstTimePredicted,
    server: bool,
    spartan: Option<&Spartan>,
) {
    let Some(active) = inv.active else { return };
    let Ok((weapon, hw, mut gun, _)) = guns.get_mut(active) else { return };
    let g = hw.0.clone();
    if hands.melee > 0 {
        if hands.melee == hands.melee_hit {
            // The hit: 25 probes from the eye, the nearest character.
            let rewound = if server && first.0 { lag.rewind(owner, true, None) } else { Vec::new() };
            let p = aim.any_orthonormal_vector();
            let c = aim.cross(p);
            let mut best: Option<(f32, Entity)> = None;
            for i in -2..=2 {
                for j in -2..=2 {
                    let probe = (aim * 0.8 + (c * i as f32 + p * j as f32) * 0.1) * METERS_PER_WU;
                    if let Some((d, _, _, Some((e, _)), _)) = trace.trace(eye, eye + probe, &[owner], &rewound)
                        && best.is_none_or(|b| d < b.0)
                    {
                        best = Some((d, e));
                    }
                }
            }
            let line = (eye, eye + aim * 0.8 * METERS_PER_WU);
            event(events, first, owner, active, WeaponEventKind::Swing {
                hit: best.is_some(),
                secondary: false,
                at: best.map(|(d, e)| (eye + aim * d, -aim, e)),
                line,
            });
            if server
                && first.0
                && let Some((_, target)) = best
                && let Some(effect) = g.melee_effect()
                && let Some(fx) = out.effects.get(effect)
            {
                let forward = velocity.dot(aim) / METERS_PER_WU / 30.0;
                let scale = spartan.map_or(1.0, |s| melee_scale(s, forward, hands.airborne));
                let mut rng = HaloRng(gun.rng);
                let u = rng.real();
                gun.rng = rng.0;
                out.hits.write(HaloHit::melee(target, effect, fx.amount(scale, u), scale, eye, aim, Some(owner)).weapon(weapon.id));
            }
        }
        hands.melee -= 1;
    } else if hands.melee_button && !g.has(weapon_flags::PREVENTS_MELEE) && gun.zoom < 0 && !gun.charging() && !hands.throw.busy() {
        let (length, hit) = g.melee_timing();
        hands.melee = length;
        hands.melee_hit = hit;
        if let Some(GunEvent::RoundsIn { .. }) = gun.settle_reload(&g) {
            event(events, first, owner, active, WeaponEventKind::Reloaded);
        }
        event(events, first, owner, active, WeaponEventKind::Action("melee"));
    }
}

/// A Halo character hurt drops out of its zoom (weapons.md 12).
pub fn unzoom_when_hurt(mut hurt: MessageReader<HaloHurt>, holders: Query<&Inventory>, mut guns: Query<&mut HaloGun>) {
    for h in hurt.read() {
        if h.outcome.shield_taken <= 0.0 && h.outcome.body_amount <= 0.0 {
            continue;
        }
        if let Some(mut g) = holders.get(h.target).ok().and_then(|i| i.active).and_then(|a| guns.get_mut(a).ok()) {
            g.zoom = -1;
        }
    }
}

/// A shot's camouflage loss on its shooter (weapons.md 17; server).
pub fn camo_on_fire(mut fired: MessageReader<HaloFired>, guns: Query<&HaloWeapon>, mut vitality: Query<&mut Vitality>, spartan: Option<Res<Spartan>>) {
    let Some(spartan) = spartan else { return };
    for f in fired.read().filter(|f| !f.misfired) {
        if let (Ok(hw), Ok(mut v)) = (guns.get(f.weapon), vitality.get_mut(f.owner)) {
            v.fired(&spartan, Some(hw.0.camo[0]), hw.0.camo[1]);
        }
    }
}
