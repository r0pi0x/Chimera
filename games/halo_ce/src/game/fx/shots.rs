//! What a Halo shot shows and sounds like, on every machine
//! (specs/halo_ce/effects.md "What is on the disc", projectiles.md 17,
//! weapons.md's firing effects):
//!
//! - **The shot** (`on_fired`, from `HaloFired`: the shooter's predicted
//!   one, or another player's as the server sent it, `HaloShotNet`): the
//!   firing effect of its entry at the gun's markers (the first person's
//!   for the local shooter, the third person's for everyone else), the
//!   muzzle light (the weapon's light attachment: its colour and radius,
//!   dimming over 1 / (30 x recovery) ticks: three for the bullets' guns),
//!   the kick (the trigger's firing damage effect's shake: `ViewPunch` of
//!   its rotation in a random direction for its ticks, on the local
//!   shooter, while Halo's recoil owns the punch), and each projectile's
//!   visible flight.
//! - **The flight** (`HaloShown`, `fly`): each projectile of all nine guns
//!   flown again from the shot by `weapons::flight` (the generalised rules:
//!   the response per hit by the hit's material type) one update a Halo
//!   tick against the level and the characters, with no damage (the
//!   server's own are the real ones): the trace is the server's (the
//!   level's material type from the arsenal's table, the collision
//!   radius's sideways probes; a character is the cyborg's, its shield's
//!   while up), the response decision is the flight's own, and its
//!   random draws come from the shown projectile's own generator
//!   (`shown_seed`), never the simulation's: the plasma bolt is a glow at its point each tick (its
//!   contrail's look, its light's colour) and its trail; a tracer round
//!   (the weapon's rounds-between-tracers rule, the shot's flag) draws its
//!   contrail's streak over the tick's segment; pistol bullets and pellets
//!   have no contrail and show only where they hit. Where it hits, the
//!   impact effect of the projectile's response for the surface's material
//!   (the hit's own material type where the map has the arsenal's table;
//!   else `material_of`: Halo's surface classes to its material types; a
//!   character: its shield's flare while its shield is up, else the
//!   cyborg body's), with the impact markers ("normal", "incident",
//!   "negative incident", "reflection", "gravity"). The authority places
//!   the decals (they reach the clients as the engine's decals do).
//! - **Other events** (`on_events`): the empty click's effect, the
//!   overheat's, the zoom's sounds; the first-person graph's sounds at
//!   their frames (ready, reloads, melee, the shotgun's steps and pump,
//!   the overheat and its exit) for the local holder.
//! - **Other games' guns on a Halo-looking map** (`other_impacts`): where
//!   the engine's impact route says Halo draws an impact (the map's game,
//!   `weapon::impact`), the assault rifle bullet's impact effect for the
//!   surface hit.
//!
//! Where on the gun (`place::Placer`): the shooter sees its effects at its
//! view model's animated markers, everyone else at its held model's; an
//! effect on a gun is placed on it again each tick it runs
//! (`step_effects`), and the muzzle light goes with its shooter
//! (`HaloLight::follow`).
//!
//! The whole arsenal: a shot flies its trigger's projectile (the plasma
//! pistol's second trigger: the charged bolt) from the eye moved by the
//! trigger's first-person offset, as the server's (a gun whose muzzle is
//! data, `load::FxGun::shown_from_muzzle`, the Hunter's cannon: drawn
//! leaving from its muzzle, parallel to the server's from the eye); a guided one (the
//! needles, the charged bolt) is steered toward the shot's target
//! (`HaloFired::target`, the server's pick on the shooter's machine and in
//! `HaloShotNet`) by the server's rule (the guided rate, half against a
//! player, the wander at the shot's tick); its look is its projectile's
//! (`load::FxProjectile`: tracer, glow, impacts; the gun's for a first
//! trigger without one), a detonation on a hit plays that material's
//! potential response's effect (the charged bolt's "charged bolt impact");
//! a projectile with a model (the rocket, a needle) is drawn as a shown
//! item facing its flight (`objects::trails` adds its lights and attached
//! effects); a needle that sticks stays where it stuck (in a character it
//! moves with it) until its detonation (`objects`) or `STUCK_TICKS`.
//!
//! A long-lived trail's points (the rocket's) live through the contrail's
//! point states, each point drawing its own lengths, quantised to the 30 Hz
//! updates (`point_lengths`).
//!
//! Known shortcuts: a contrail point's density scale flags read a density
//! of 1; the server's response draws depend on the weapon's own
//! generator, which the shot does not carry, so a shown projectile uses a
//! fixed seed per projectile of the shot: only a response with a random
//! skip fraction (and the timer's draw, 0 here) can differ from the real
//! flight; the wander's tick is the shot's arrival here (off by the link's
//! delay on another client); the shooter's inherited velocity is not
//! added; a shown projectile meets characters where they are drawn on that
//! machine; the
//! kick's direction is drawn from the engine's particle generator; others'
//! empty clicks and overheats are not sent (the shooter's own show).

use std::{collections::BTreeMap, sync::Arc};

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use chimera_sim::weapon::{
    ViewPunch, Weapon, WeaponEvent, WeaponEventKind,
    impact::{ImpactBy, ImpactRoute, RecoilGame},
};
use serde::{Deserialize, Serialize};

use super::{
    ActiveFx, HaloLight,
    load::{FxGun, FxLight, FxTracer, FxTracerState, HaloFx},
    place::{self, Frame, Placer},
    play::{Effects, Instance, Made, Running, start},
};
use super::super::{
    GAME, METERS_PER_UNIT,
    vitality::Vitality,
    view::load::object_key,
    weapons::{
        GUNS, HaloFired, HaloGun,
        flight::{End, Hit, Steer, wandered},
        halo_ticks,
        params::{Gun, Projectile},
        random::HaloRng,
        systems::{GUIDANCE_VS_PLAYER, HaloTrace},
    },
};
use crate::{
    core::{Intent, LocalPlayer, MovementState, NetRole, SimClock, SimSet},
    map::{
        PlaceDecal, PlaySound,
        loose::ShownItem,
        impact::{ActiveImpacts, SurfaceClass, SurfaceClasses},
        particles::{Fade, Particle, ParticleCurves, ParticleGroup, Particles, Ramp, Shape},
        sound::SurfaceGrid,
    },
};

/// Halo's material type codes (collision.md) by the engine's surface
/// class (its maps' surfaces are classes: `maps::surfaces`).
pub fn material_of(class: SurfaceClass) -> usize {
    match class {
        SurfaceClass::Dirt => 0,
        SurfaceClass::Sand => 1,
        SurfaceClass::Snow => 3,
        SurfaceClass::Wood => 4,
        SurfaceClass::Metal => 6,
        SurfaceClass::Plastic => 8,
        SurfaceClass::Glass => 9,
        SurfaceClass::Flesh => 24,
        SurfaceClass::Water => 28,
        SurfaceClass::Grass => 29,
        _ => 2,
    }
}

/// The cyborg body and its shield (collision.md).
pub const CYBORG: usize = 21;
pub const CYBORG_SHIELD: usize = 22;

/// Each gun's numbers by id (once per app).
#[derive(Resource, Clone)]
pub struct Guns(pub BTreeMap<&'static str, Arc<Gun>>);

impl Default for Guns {
    fn default() -> Self {
        let all = super::super::weapons::guns();
        Self(GUNS.iter().filter_map(|id| all.iter().find(|(i, _)| i == id).map(|(_, g)| (*id, Arc::new(g.clone())))).collect())
    }
}

/// A projectile flown again for its look (module doc).
#[derive(Component, Clone, Debug)]
pub struct HaloShown {
    pub gun: Arc<Gun>,
    /// The projectile it is (the trigger's: the plasma pistol's charged
    /// bolt is its second's).
    pub proj: Arc<Projectile>,
    pub id: &'static str,
    pub owner: Entity,
    pub flight: super::super::weapons::flight::Flight,
    pub ignored: Option<Entity>,
    pub tracer: bool,
    /// Its own random generator's state (module doc: never the server's).
    pub rng: u32,
    /// Its updates so far.
    pub updates: u32,
    /// A guided one's target (the shot's) and its wander's numbers.
    pub target: Option<Entity>,
    pub tick: u64,
    pub k: u32,
    /// Stuck (a needle): in a character at an offset from its centre
    /// (metres), or in the level; with the Halo ticks left to show it.
    pub stuck: Option<(Option<Entity>, Vec3, u32)>,
}

/// The longest a stuck needle is shown without its detonation (Halo
/// ticks): a needle's own timer is under one second.
pub const STUCK_TICKS: u32 = 150;

/// A shown projectile's look: its projectile's (by tag), else for the
/// first trigger its gun's (the tracer, the glow, the impacts).
#[derive(Clone, Copy, Debug)]
pub struct Look<'a> {
    pub tracer: Option<&'a FxTracer>,
    pub light: Option<&'a FxLight>,
    pub impacts: &'a [Option<usize>],
    pub potential: &'a [Option<usize>],
    pub model: bool,
}

/// `Look` of the projectile `proj` fired by `gun`.
pub fn look_of<'a>(lib: &'a HaloFx, gun: &Gun, proj: &Projectile) -> Option<Look<'a>> {
    if let Some(p) = lib.projectile(&proj.tag) {
        return Some(Look { tracer: p.tracer.as_ref(), light: p.light.as_ref(), impacts: &p.impacts, potential: &p.potential, model: p.model });
    }
    let g = lib.gun(&gun.tag).filter(|_| gun.projectile.tag.eq_ignore_ascii_case(&proj.tag))?;
    Some(Look { tracer: g.tracer.as_ref(), light: g.bolt_light.as_ref(), impacts: &g.impacts, potential: &[], model: false })
}

/// The projectile a gun's trigger fires.
pub fn trigger_projectile(gun: &Gun, trigger: u8) -> &Projectile {
    match (&gun.secondary, trigger) {
        (Some(s), 1) => &s.projectile,
        _ => &gun.projectile,
    }
}

/// Server -> every client but the shooter's: a Halo shot (`HaloFired`).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, bevy::ecs::entity::MapEntities)]
pub struct HaloShotNet {
    #[entities]
    pub owner: Entity,
    /// Index into `GUNS`.
    pub gun: u8,
    pub effect: u8,
    pub misfired: bool,
    pub last_round: bool,
    pub origin: [f32; 3],
    pub projectiles: Vec<([f32; 3], bool)>,
    /// The trigger (1: the plasma pistol's charged bolt).
    pub trigger: u8,
    /// A guided projectile's target (the needles', the charged bolt's).
    #[entities]
    pub target: Option<Entity>,
}

/// The random generator's seed of a shown projectile: one fixed seed per
/// projectile of the shot (the server's depends on the weapon's own
/// generator, which the shot message does not carry).
pub fn shown_seed(i: usize) -> u32 {
    0x5EED_5409 ^ (i as u32).wrapping_mul(0x9E37_79B9)
}

/// The kick left on a shooter: its rotation (rad) and ticks; on every
/// character from the spawn, `active` while one is left (a part that came
/// and went would make archetypes: docs/DESIGN.md, "ECS style").
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HaloKick {
    pub rotation: f32,
    pub ticks: u32,
    pub active: bool,
}

/// Animation sounds waiting for their frame: (ticks left, entry, at whom).
#[derive(Resource, Default)]
pub struct PendingSounds(pub Vec<(u32, String, Entity)>);

/// A level trace (engine space): the fraction along `from -> to` and the
/// normal of the first level surface.
fn level_trace<'a>(spatial: &'a SpatialQuery, characters: &'a Query<Entity, With<Intent>>) -> impl Fn(Vec3, Vec3) -> Option<(f32, Vec3)> + 'a {
    move |from: Vec3, to: Vec3| {
        let (dir, len) = Dir3::new_and_length(to - from).ok()?;
        let filter = SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(crate::core::SOLID_LAYERS);
        spatial.cast_ray(from, dir, len, true, &filter).map(|h| (h.distance / len, h.normal))
    }
}

/// The impact markers at `point` (projectiles.md 17): normal, incident,
/// negative incident, reflection, gravity; `v` the projectile's direction.
pub fn impact_frames(point: Vec3, normal: Vec3, v: Vec3) -> BTreeMap<&'static str, Frame> {
    let v = v.normalize_or(-normal);
    let r = v - 2.0 * v.dot(normal) * normal;
    BTreeMap::from([
        ("normal", Frame::facing(point, normal)),
        ("incident", Frame::facing(point, -v)),
        ("negative incident", Frame::facing(point, v)),
        ("reflection", Frame::facing(point, r)),
        ("gravity", Frame::facing(point, Vec3::NEG_Y)),
    ])
}

/// An effect's instances from named frames (a name the list lacks takes
/// its first, as the spec says of a caller's marker list).
pub fn instances_from(fx: &HaloFx, effect: usize, frames: &BTreeMap<&'static str, Frame>, first: Frame) -> Vec<Vec<Instance>> {
    fx.effects[effect]
        .locations
        .iter()
        .map(|l| vec![Instance::both(frames.get(l.to_ascii_lowercase().as_str()).copied().unwrap_or(first))])
        .collect()
}

/// An effect's locations as marker names (lower case).
pub fn markers_of(fx: &HaloFx, effect: usize) -> Vec<String> {
    fx.effects[effect].locations.iter().map(|l| l.to_ascii_lowercase()).collect()
}

/// A gun's effect at `owner`'s gun's markers `names`: each instance as the
/// shooter sees it (the view model's) and as everyone else does (the held
/// model's); a marker with several instances is all of them.
pub fn gun_instances(names: &[String], gun: &FxGun, camera: Frame, placer: &Placer, owner: Entity) -> Vec<Vec<Instance>> {
    names
        .iter()
        .map(|name| {
            let (first, third) = (placer.first(owner, camera, gun, name), placer.third(owner, camera, gun, name));
            (0..first.len().max(third.len()))
                .map(|i| Instance { first: first[i % first.len()], third: third[i % third.len()] })
                .collect()
        })
        .collect()
}

/// A holder's eye: its transform and the movement's eye offset.
fn eye_of(at: &Transform, ms: Option<&MovementState>) -> Vec3 {
    at.translation + ms.map_or(Vec3::ZERO, |m| m.eye_offset)
}

/// Who sees a shot in first person: the local player (or, with no network
/// and no local player, a test's shooter).
fn first_person(owner: Entity, locals: &Query<(), With<LocalPlayer>>, role: Option<&NetRole>) -> bool {
    locals.contains(owner) || (role.is_none() && locals.is_empty())
}

/// The shots' presentation (module doc).
#[allow(clippy::too_many_arguments)]
fn on_fired(
    mut fired: MessageReader<HaloFired>,
    fx: Res<ActiveFx>,
    guns: Res<Guns>,
    mut effects: ResMut<Effects>,
    holders: Query<&Intent>,
    locals: Query<(), With<LocalPlayer>>,
    role: Option<Res<NetRole>>,
    mut lights: MessageWriter<HaloLight>,
    placer: Placer,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let authority = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for f in fired.read() {
        let Some(gun) = guns.0.get(f.id) else { continue };
        let (trigger, proj) = match (&gun.secondary, f.trigger) {
            (Some(s), 1) => (&s.trigger, Arc::new(s.projectile.clone())),
            _ => (&gun.trigger, Arc::new(gun.projectile.clone())),
        };
        // From the eye moved by the trigger's first-person offset, as the
        // server's (`weapons::systems`).
        let aim = f.projectiles.first().map_or(Vec3::NEG_Z, |p| p.0);
        let mut origin = f.origin / METERS_PER_UNIT;
        if !trigger.has(super::super::weapons::params::trigger_flags::PROJECTILE_VECTOR_CANNOT_BE_ADJUSTED) {
            let left = Vec3::Y.cross(aim).try_normalize().unwrap_or(Vec3::Y);
            let up = aim.cross(left);
            let [x, y, z] = trigger.first_person_offset;
            origin += aim * x + left * y + up * z;
        }
        // A gun whose muzzle is data (`load::FxGun::shown_from_muzzle`, a
        // campaign character's own): its shots drawn leaving from there.
        let view = holders.get(f.owner).map_or(Quat::IDENTITY, |i| chimera_sim::weapon::aim_of(i.yaw, i.pitch));
        let camera = place::camera(f.origin, view);
        let fp = first_person(f.owner, &locals, role.as_deref());
        if let Some(look) = fx.0.as_ref().and_then(|l| l.gun(&gun.tag)).filter(|g| g.shown_from_muzzle) {
            let at = if fp { placer.first(f.owner, camera, look, "primary trigger") } else { placer.third(f.owner, camera, look, "primary trigger") };
            if let Some(m) = at.first() {
                origin = m.at / METERS_PER_UNIT;
            }
        }
        let model = fx.0.as_ref().and_then(|l| look_of(l, gun, &proj)).is_some_and(|l| l.model);
        for (i, (dir, tracer)) in f.projectiles.iter().enumerate() {
            let flight = super::super::weapons::flight::Flight::new(&proj, origin, *dir, 0.0);
            let mut e = commands.spawn((
                HaloShown {
                    gun: gun.clone(),
                    proj: proj.clone(),
                    id: f.id,
                    owner: f.owner,
                    flight,
                    ignored: None,
                    tracer: *tracer,
                    rng: shown_seed(i),
                    updates: 0,
                    target: f.target,
                    tick: clock.tick,
                    k: (clock.tick as u32).wrapping_add(i as u32),
                    stuck: None,
                },
                Name::new("halo_ce shown projectile"),
            ));
            // Its model (the rocket, a needle), drawn where it flies.
            if model {
                e.insert((ShownItem(object_key(&proj.tag)), facing(origin * METERS_PER_UNIT, *dir)));
            }
        }
        let Some(lib) = fx.0.clone() else { continue };
        let Some(look) = lib.gun(&gun.tag) else { continue };
        let list = if f.misfired { &look.misfire } else { &look.firing };
        // The library lists every trigger's entries in order: the second
        // trigger's (the charged bolt's) after the first's.
        let entry = f.effect as usize + if f.trigger == 1 { gun.trigger.firing_effects as usize } else { 0 };
        if let Some(effect) = list.get(entry).copied().flatten() {
            let names = markers_of(&lib, effect);
            let inst = gun_instances(&names, look, camera, &placer, f.owner);
            start(&mut effects, &lib, effect, inst, |r| {
                r.first_person = fp;
                r.owner = Some(f.owner);
                r.decals = authority;
                r.on_gun = Some((gun.tag.clone(), names));
            });
        }
        if !f.misfired && let Some((light, marker)) = &look.light {
            let marker = marker.to_ascii_lowercase();
            let at = if fp { placer.first(f.owner, camera, look, &marker) } else { placer.third(f.owner, camera, look, &marker) };
            lights.write(HaloLight {
                at: at[0].at,
                color: Vec3::from(light.color),
                radius: light.radius * METERS_PER_UNIT,
                seconds: muzzle_light_ticks(gun) / 30.0,
                falloff: 0,
                owner: Some(f.owner),
                follow: true,
            });
        }
        if fp && let Some((rotation, ticks)) = look.kick {
            commands.entity(f.owner).insert(HaloKick { rotation, ticks, active: true });
        }
    }
}

/// A shown object's transform at `at` (metres) moving along `dir`: its
/// model's +X (Halo's forward, the engine's -Z in its own frame) along it.
pub fn facing(at: Vec3, dir: Vec3) -> Transform {
    let d = dir.normalize_or(Vec3::NEG_Z);
    Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, d))
}

/// The muzzle light's ticks: its intensity falls from 1 by the weapon's
/// illumination recovery a tick (effects.md "The muzzle flash light": 3
/// ticks for the bullets' guns, 4.5 for the plasma rifle).
pub fn muzzle_light_ticks(gun: &Gun) -> f32 {
    if gun.tag.to_ascii_lowercase().contains("plasma rifle") { 4.5 } else { 3.0 }
}

/// The kick on its shooter's punch, while Halo's recoil owns it.
fn kick(mut shooters: Query<(Entity, &mut HaloKick, Option<&mut ViewPunch>, Option<&RecoilGame>)>, mut effects: ResMut<Effects>, clock: Res<SimClock>, mut commands: Commands) {
    if halo_ticks(&clock) == 0 {
        return;
    }
    for (e, mut k, punch, game) in &mut shooters {
        if !k.active {
            continue;
        }
        if !RecoilGame::is(game, GAME) {
            k.active = false;
            continue;
        }
        let v = if k.ticks > 0 {
            let a = effects.rng.random() * std::f32::consts::TAU;
            Vec3::new(a.sin(), a.cos(), 0.0) * k.rotation
        } else {
            Vec3::ZERO
        };
        match punch {
            Some(mut p) => p.0 = v,
            None => {
                commands.entity(e).insert(ViewPunch(v));
            }
        }
        if k.ticks == 0 {
            k.active = false;
        } else {
            k.ticks -= 1;
        }
    }
}

/// The struck character's material: its shield while up, else the body.
fn character_material(target: Entity, vitality: &Query<&Vitality>) -> usize {
    if vitality.get(target).is_ok_and(|v| v.shield > 0.0) { CYBORG_SHIELD } else { CYBORG }
}

/// The surface class at a level point (the map's surface there, by the
/// map's game's names).
fn class_at(point: Vec3, grid: Option<&SurfaceGrid>, classes: Option<&SurfaceClasses>, impacts: Option<&ActiveImpacts>) -> SurfaceClass {
    let Some(name) = grid.and_then(|g| g.nearest(point, 0.1)) else { return SurfaceClass::Default };
    match classes {
        Some(c) => c.classify(impacts.and_then(|i| i.0.map_game()), name),
        None => SurfaceClass::guess(name),
    }
}

/// The shown projectiles, one update a Halo tick (module doc).
#[allow(clippy::too_many_arguments)]
fn fly(
    mut shown: Query<(Entity, &mut HaloShown, Option<&mut Transform>), Without<Intent>>,
    trace: HaloTrace,
    vitality: Query<&Vitality>,
    fx: Res<ActiveFx>,
    mut effects: ResMut<Effects>,
    mut particles: ResMut<Particles>,
    grid: Option<Res<SurfaceGrid>>,
    classes: Option<Res<SurfaceClasses>>,
    impacts: Option<Res<ActiveImpacts>>,
    role: Option<Res<NetRole>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    let authority = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    let lib = fx.0.clone();
    for (e, mut s, mut at) in &mut shown {
        for _ in 0..n {
            let s = &mut *s;
            // Stuck (a needle): it stays where it stuck, with what holds it.
            if let Some((target, offset, left)) = s.stuck {
                let point = match target {
                    Some(t) => trace.any_centre(t).map(|c| c + offset),
                    None => Some(offset),
                };
                match point.filter(|_| left > 0) {
                    Some(p) => {
                        s.stuck = Some((target, offset, left - 1));
                        if let Some(t) = at.as_deref_mut() {
                            t.translation = p;
                        }
                    }
                    None => {
                        commands.entity(e).despawn();
                        break;
                    }
                }
                continue;
            }
            let gun = s.gun.clone();
            let proj = s.proj.clone();
            let owner = s.owner;
            let radius = proj.collision_radius;
            let mut tracer = |a: Vec3, b: Vec3, ignored: Option<Entity>| -> Option<Hit<Entity>> {
                let skip: Vec<Entity> = std::iter::once(owner).chain(ignored).collect();
                let (am, bm) = (a * METERS_PER_UNIT, b * METERS_PER_UNIT);
                let mut met = trace.trace(am, bm, &skip, &[]);
                // As the server's: a collision radius widens against
                // characters only, sideways, when the line met nothing.
                if met.is_none() && radius >= 0.0001 {
                    let l = Vec3::Y.cross(b - a).try_normalize().unwrap_or(Vec3::NEG_X) * radius * METERS_PER_UNIT;
                    met = [l, -l].into_iter().find_map(|o| trace.trace_with(am + o, bm + o, &skip, &[], false));
                }
                let (_, point, normal, what, material) = met?;
                let len = (bm - am).length();
                let t = if len > 0.0 { ((point - am).length() / len).clamp(0.0, 1.0) } else { 0.0 };
                let point = a + (b - a) * t;
                // As the server's: a prop struck (no region) keeps its
                // material and responds as the level.
                let character = matches!(what, Some((_, Some(_))));
                Some(Hit {
                    t,
                    point,
                    normal,
                    object: what.map(|w| w.0),
                    material: if character { Some(CYBORG as u8) } else { material },
                    unit: character,
                })
            };
            // The server's rule without its damage roll: a projectile that
            // can hurt meets the shield or the body by the shield's state.
            let hurts = proj.impact_effect().is_some();
            let mut struck = |h: &Hit<Entity>, _: Vec3, _: f32| -> Option<u8> {
                let target = h.object?;
                if !h.unit {
                    return hurts.then_some(h.material).flatten();
                }
                hurts.then(|| if trace.shielded(target) { CYBORG_SHIELD as u8 } else { CYBORG as u8 })
            };
            // Guided (the needles, the charged bolt): toward the shot's
            // target as the server steers it.
            let steer = s.target.filter(|_| proj.guided_angular_velocity > 0.0).and_then(|t| trace.centre(t)).map(|c| {
                let to = c / METERS_PER_UNIT;
                let d = (to - s.flight.position).length();
                Steer { target: wandered(to, d, s.tick + u64::from(s.updates), s.k), omega: proj.guided_angular_velocity / 30.0 * GUIDANCE_VS_PLAYER }
            });
            let mut ignored = s.ignored;
            let mut rng = HaloRng(s.rng);
            let moved = s.flight.update(&proj, &mut ignored, &mut rng, steer, &mut tracer, &mut struck);
            s.rng = rng.0;
            s.ignored = ignored;
            s.updates += 1;
            if let Some(t) = at.as_deref_mut() {
                *t = facing(s.flight.position * METERS_PER_UNIT, s.flight.velocity);
            }
            if let Some(lib) = &lib {
                let look = look_of(lib, &gun, &proj);
                let last = moved.segments.iter().rposition(|x| x.2.is_some());
                for (k, (from, to, hit)) in moved.segments.iter().enumerate() {
                    let (a, b) = (*from * METERS_PER_UNIT, *to * METERS_PER_UNIT);
                    if let Some(look) = look {
                        draw_segment(&mut particles, look.tracer, look.light, a, b, s.tracer, s.updates == 1);
                    }
                    let Some(h) = hit else { continue };
                    let material = match h.object {
                        Some(t) => character_material(t, &vitality),
                        // The level's own material type, where the map has
                        // the table; else the surface's class.
                        None if trace.has_materials() => match h.material {
                            Some(m) => m as usize,
                            None => continue,
                        },
                        None => material_of(class_at(b, grid.as_deref(), classes.as_deref(), impacts.as_deref())),
                    };
                    // A detonation on this hit took the potential response
                    // (the charged bolt's "charged bolt impact").
                    let potential = moved.end == Some(End::Detonate) && Some(k) == last;
                    let effect = look.and_then(|l| {
                        let p = if potential { l.potential.get(material).copied().flatten() } else { None };
                        p.or_else(|| l.impacts.get(material).copied().flatten())
                    });
                    let Some(effect) = effect else { continue };
                    let frames = impact_frames(b, h.normal, b - a);
                    let inst = instances_from(lib, effect, &frames, frames["normal"]);
                    start(&mut effects, lib, effect, inst, |r| {
                        r.owner = Some(owner);
                        r.decals = authority && h.object.is_none();
                    });
                }
            }
            if let Some(h) = moved.stuck {
                let point = h.point * METERS_PER_UNIT;
                let held = h.object.and_then(|t| trace.any_centre(t).map(|c| (t, c)));
                s.stuck = Some(match held {
                    Some((t, c)) => (Some(t), point - c, STUCK_TICKS),
                    None => (None, point, STUCK_TICKS),
                });
                if let Some(t) = at.as_deref_mut() {
                    t.translation = point;
                }
                continue;
            }
            if matches!(moved.end, Some(End::Detonate | End::Disappear)) {
                commands.entity(e).despawn();
                break;
            }
        }
    }
}

/// A tick's segment of a shown projectile: the tracer's streak (its
/// contrail's width and colour over the segment, gone by the next tick)
/// or, for a projectile with a light (the plasma bolt, a needle), its glow
/// at the segment's end and its trail.
pub fn draw_segment(particles: &mut Particles, tracer: Option<&FxTracer>, light: Option<&FxLight>, a: Vec3, b: Vec3, is_tracer: bool, first: bool) {
    let Some(t) = tracer else { return };
    let Some(material) = t.material else { return };
    if !is_tracer && light.is_none() {
        return;
    }
    let _ = first;
    let width = t.width * METERS_PER_UNIT;
    let color = Vec3::new(t.color[0], t.color[1], t.color[2]);
    // A point lasts its contrail's life (the rocket's smoke trail stays
    // and widens; a bullet's streak is gone by the next tick), fading.
    let life = t.life.max(1.0 / 30.0);
    let long = life > 1.5 / 30.0;
    let mut g = ParticleGroup::new(Default::default());
    g.capped = false;
    g.kill_after = Some(life + 0.004);
    // Its first state's width (the states' widening is not drawn).
    let wide = width;
    // A long-lived trail's points live through the contrail's states: their
    // colour and alpha, and the ribbon's width, follow them over the life,
    // each point with its own lengths (`point_lengths`).
    let mut life = life;
    let curves = (long && t.states.len() > 1).then(|| {
        let rng = &mut particles.rng;
        let lengths = point_lengths(&t.states, 1.0 / 30.0, || rng.random());
        let total: f32 = lengths.iter().map(|(s, tr)| s + tr).sum::<f32>().max(1.0 / 30.0);
        life = total;
        let (mut at, mut color_keys, mut size_keys) = (0.0f32, Vec::new(), Vec::new());
        for (s, (state, transition)) in t.states.iter().zip(&lengths) {
            for end in [at, at + state] {
                let f = end / total;
                color_keys.push((f, [s.color[1], s.color[2], s.color[3], s.color[0]]));
                size_keys.push((f, s.width * METERS_PER_UNIT));
            }
            at += state + transition;
        }
        std::sync::Arc::new(ParticleCurves { color: color_keys, size: size_keys, ..Default::default() })
    });
    g.kill_after = Some(life + 0.004);
    let has_curves = curves.is_some();
    let mut streak = Particle {
        color,
        curves,
        alpha: if has_curves {
            1.0
        } else if long { t.color[3].clamp(0.0, 1.0) * 0.5 } else { t.color[3].max(0.5) },
        fade: if long { Fade::Ramp(Ramp::linear(1.0, 0.0)) } else { Fade::Ramp(Ramp::constant(1.0)) },
        shape: Shape::Ribbon { to: b, width: if long { [wide, width] } else { [width * 0.25, width] }, u: (0.0, 1.0), end_color: None, prev: None, next: None },
        ..Particle::new(a, life, material, width)
    };
    if let Some(l) = light {
        streak.color = Vec3::from(l.color);
        let glow = Particle { color: Vec3::from(l.color), ..Particle::new(b, 1.0 / 30.0, material, (width * 2.0).max(0.12)) };
        g.particles.push(glow);
    }
    g.particles.push(streak);
    particles.add(g);
}

/// One contrail point's lengths (effects.md "Contrails", point life): per
/// state, the seconds it spends in that state and in the transition to the
/// next (none after the last state, whose own length ends the point's
/// life). Each length is the point's own draw, `lower + U(0, upper -
/// lower)` with `draw` the local generator's U(0, 1) (made even when the
/// bounds are equal; the density scale flags are taken at a density of
/// 1); it lasts the updates of `dt` until the progress `dt / length`
/// summed passes 1 strictly, the overshoot dropped (a length of 0 passes
/// at once).
pub fn point_lengths(states: &[FxTracerState], dt: f32, mut draw: impl FnMut() -> f32) -> Vec<(f32, f32)> {
    let n = states.len();
    states
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let state = updates_in(point_length(s.duration_bounds, draw()), dt) as f32 * dt;
            let transition = if k + 1 < n { updates_in(point_length(s.transition_bounds, draw()), dt) as f32 * dt } else { 0.0 };
            (state, transition)
        })
        .collect()
}

/// A point's length drawn from `(lower, upper)` with the draw `u` (0 to 1).
pub fn point_length((lower, upper): (f32, f32), u: f32) -> f32 {
    lower + (upper - lower).max(0.0) * u
}

/// How many updates of `dt` a stage of `length` seconds lasts: until its
/// progress (dt / length a update) passes 1 strictly; 0 for a length of 0.
pub fn updates_in(length: f32, dt: f32) -> u32 {
    if length <= 0.0 || dt <= 0.0 {
        return 0;
    }
    let (mut progress, mut n) = (0.0f32, 0);
    while progress <= 1.0 && n < 100_000 {
        progress += dt / length;
        n += 1;
    }
    n
}

// Step the running effects a Halo tick and write what they made.
#[allow(clippy::too_many_arguments)]
fn step_effects(
    fx: Res<ActiveFx>,
    mut effects: ResMut<Effects>,
    spatial: SpatialQuery,
    characters: Query<Entity, With<Intent>>,
    clock: Res<SimClock>,
    mut particles: ResMut<Particles>,
    mut sounds: MessageWriter<PlaySound>,
    mut decals: MessageWriter<PlaceDecal>,
    mut lights: MessageWriter<HaloLight>,
    holders: Query<(&Intent, &Transform, Option<&MovementState>)>,
    placer: Placer,
) {
    let Some(lib) = fx.0.clone() else { return };
    let world = level_trace(&spatial, &characters);
    let mut made = Made::default();
    let n = halo_ticks(&clock);
    let Effects { running, rng } = &mut *effects;
    for r in running.iter_mut() {
        // On a holder's gun: its markers where the gun is now.
        if let (Some(owner), Some((tag, names))) = (r.owner, &r.on_gun)
            && let (Some(look), Ok((intent, at, ms))) = (lib.gun(tag), holders.get(owner))
        {
            let camera = place::camera(eye_of(at, ms), chimera_sim::weapon::aim_of(intent.yaw, intent.pitch));
            r.instances = gun_instances(names, look, camera, &placer, owner);
        }
        // Made this tick: its creation update (dt 0), then ticks.
        r.step(&lib, 0.0, rng, &world, &mut made);
        for _ in 0..n {
            r.step(&lib, 1.0 / 30.0, rng, &world, &mut made);
        }
    }
    running.retain(|r: &Running| !r.done);
    sounds.write_batch(made.sounds);
    decals.write_batch(made.decals);
    lights.write_batch(made.lights);
    for g in made.groups {
        particles.add(g);
    }
}

/// Weapon events' sounds and effects (module doc).
#[allow(clippy::too_many_arguments)]
fn on_events(
    mut events: MessageReader<WeaponEvent>,
    fx: Res<ActiveFx>,
    weapons: Query<(&Weapon, Option<&HaloGun>)>,
    guns: Res<Guns>,
    holders: Query<(&Intent, &Transform, Option<&MovementState>)>,
    locals: Query<(), With<LocalPlayer>>,
    role: Option<Res<NetRole>>,
    mut effects: ResMut<Effects>,
    mut pending: ResMut<PendingSounds>,
    mut play: MessageWriter<PlaySound>,
    placer: Placer,
) {
    let Some(lib) = fx.0.clone() else {
        events.clear();
        return;
    };
    for ev in events.read().filter(|e| !e.replay) {
        let Ok((w, state)) = weapons.get(ev.weapon) else { continue };
        let Some(gun) = guns.0.get(w.id) else { continue };
        let Some(look) = lib.gun(&gun.tag) else { continue };
        let Ok((intent, at, ms)) = holders.get(ev.owner) else { continue };
        let eye = eye_of(at, ms);
        let camera = place::camera(eye, chimera_sim::weapon::aim_of(intent.yaw, intent.pitch));
        let fp = first_person(ev.owner, &locals, role.as_deref());
        let effect = match &ev.kind {
            WeaponEventKind::DryFire => look.empty.first().copied().flatten(),
            WeaponEventKind::Action("overheated") => look.overheated,
            WeaponEventKind::Deployed => look.ready,
            _ => None,
        };
        if let Some(effect) = effect {
            let names = markers_of(&lib, effect);
            let inst = gun_instances(&names, look, camera, &placer, ev.owner);
            start(&mut effects, &lib, effect, inst, |r| {
                r.first_person = fp;
                r.owner = Some(ev.owner);
                r.on_gun = Some((gun.tag.clone(), names));
            });
        }
        let zoom = match &ev.kind {
            WeaponEventKind::Action("zoom_in") => look.zoom_in.as_ref(),
            WeaponEventKind::Action("zoom_out") => look.zoom_out.as_ref(),
            _ => None,
        };
        if let Some(z) = zoom {
            play.write(PlaySound { source: Some(ev.owner), ..PlaySound::at(z.clone(), eye) });
        }
        if !fp {
            continue;
        }
        let shotgun = gun.tag.to_ascii_lowercase().contains("shotgun");
        let loaded = state.map_or(1, |s| s.loaded);
        if let Some((entry, frame)) = super::sound::event_slot(&ev.kind, loaded, shotgun).and_then(|s| super::sound::event_sound(look, s)) {
            pending.0.push((frame, entry.to_string(), ev.owner));
        }
    }
}

/// The animation sounds whose frame has come (at their holder).
fn pending_sounds(mut pending: ResMut<PendingSounds>, clock: Res<SimClock>, at: Query<&Transform>, mut play: MessageWriter<PlaySound>) {
    let n = halo_ticks(&clock);
    pending.0.retain_mut(|(left, entry, owner)| {
        if *left > 0 {
            *left = left.saturating_sub(n);
            return true;
        }
        if let Ok(t) = at.get(*owner) {
            play.write(PlaySound { source: Some(*owner), ..PlaySound::at(entry.clone(), t.translation) });
        }
        false
    });
}

/// Other games' rounds that Halo's impacts draw (module doc).
#[allow(clippy::too_many_arguments)]
fn other_impacts(
    mut events: MessageReader<WeaponEvent>,
    route: ImpactRoute,
    fx: Res<ActiveFx>,
    grid: Option<Res<SurfaceGrid>>,
    classes: Option<Res<SurfaceClasses>>,
    impacts: Option<Res<ActiveImpacts>>,
    role: Option<Res<NetRole>>,
    mut effects: ResMut<Effects>,
) {
    let Some(lib) = fx.0.clone() else {
        events.clear();
        return;
    };
    let Some(look) = lib.gun("weapons\\assault rifle\\assault rifle") else { return };
    let authority = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for ev in events.read() {
        let (WeaponEventKind::Shot { from, to, hit: None, normal: Some(n) } | WeaponEventKind::ShotContinued { from, to, hit: None, normal: Some(n) }) = ev.kind else { continue };
        if route.gun_game(ev.weapon) == Some(GAME) || route.by(ev) != ImpactBy::Game(GAME) {
            continue;
        }
        let material = material_of(class_at(to, grid.as_deref(), classes.as_deref(), impacts.as_deref()));
        let Some(effect) = look.impacts.get(material).copied().flatten() else { continue };
        let frames = impact_frames(to, n, to - from);
        let inst = instances_from(&lib, effect, &frames, frames["normal"]);
        start(&mut effects, &lib, effect, inst, |r| r.decals = authority);
    }
}

/// The server's shots to the other clients (`HaloShotNet`).
fn send_shots(
    mut fired: MessageReader<HaloFired>,
    players: Query<(Entity, &chimera_net::net::server::Player)>,
    mut out: MessageWriter<bevy_replicon::prelude::ToClients<HaloShotNet>>,
) {
    use bevy_replicon::prelude::{ClientId, SendTargets, ToClients};
    for f in fired.read() {
        let Some(gun) = GUNS.iter().position(|g| *g == f.id) else { continue };
        let message = HaloShotNet {
            owner: f.owner,
            gun: gun as u8,
            effect: f.effect,
            misfired: f.misfired,
            last_round: f.last_round,
            origin: f.origin.to_array(),
            projectiles: f.projectiles.iter().map(|(d, t)| (d.to_array(), *t)).collect(),
            trigger: f.trigger,
            target: f.target,
        };
        for (client, p) in &players {
            if p.character != f.owner {
                out.write(ToClients { targets: SendTargets::Single(ClientId::Client(client)), message: message.clone() });
            }
        }
    }
}

/// A client's copy of another player's shot.
fn receive_shots(mut shots: MessageReader<HaloShotNet>, mut fired: MessageWriter<HaloFired>) {
    for s in shots.read() {
        let Some(id) = GUNS.get(s.gun as usize) else { continue };
        fired.write(HaloFired {
            owner: s.owner,
            weapon: Entity::PLACEHOLDER,
            id,
            trigger: s.trigger,
            misfired: s.misfired,
            last_round: s.last_round,
            effect: s.effect,
            origin: Vec3::from(s.origin),
            projectiles: s.projectiles.iter().map(|(d, t)| (Vec3::from(*d), *t)).collect(),
            heat: 0.0,
            age: 0.0,
            target: s.target,
        });
    }
}

fn register_net(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.add_mapped_server_message::<HaloShotNet>(Channel::Unreliable)
        .add_systems(PostUpdate, send_shots.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive_shots.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

pub(super) fn plugin(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    chimera_sim::weapon::impact::ImpactAppExt::register_impact_provider(app, GAME);
    chimera_core::character::CharacterPartsExt::on_every_character::<HaloKick>(app);
    app.init_resource::<Guns>()
        .init_resource::<PendingSounds>()
        .add_message::<HaloFired>()
        .add_message::<WeaponEvent>()
        .add_systems(
            FixedUpdate,
            (on_fired, on_events, other_impacts, fly, step_effects, pending_sounds, kick)
                .chain()
                .after(SimSet::Weapons),
        )
        .net_part("halo_ce_shots", register_net);
}
