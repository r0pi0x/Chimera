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
//! Known shortcuts: the server's response draws depend on the weapon's own
//! generator, which the shot does not carry, so a shown projectile uses a
//! fixed seed per projectile of the shot: only a response with a random
//! skip fraction (and the timer's draw, 0 here) can differ from the real
//! flight; a guided projectile (the needles, the charged bolt) is shown
//! unguided (the target is not in the shot); grenades and items are not
//! shown (server only); a shown projectile meets characters where they are
//! drawn on that machine; the muzzle light does not follow the gun; the
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
    load::{FxGun, HaloFx},
    place::{self, Frame},
    play::{Effects, Instance, Made, Running, start},
};
use super::super::{
    GAME, METERS_PER_UNIT,
    vitality::Vitality,
    weapons::{GUNS, HaloFired, HaloGun, flight::Hit, halo_ticks, params::Gun, random::HaloRng, systems::HaloTrace},
};
use crate::{
    core::{Intent, LocalPlayer, MovementState, NetRole, SimClock, SimSet},
    map::{
        PlaceDecal, PlaySound,
        impact::{ActiveImpacts, SurfaceClass, SurfaceClasses},
        particles::{Fade, Particle, ParticleGroup, Particles, Ramp, Shape},
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
    pub id: &'static str,
    pub owner: Entity,
    pub flight: super::super::weapons::flight::Flight,
    pub ignored: Option<Entity>,
    pub tracer: bool,
    /// Its own random generator's state (module doc: never the server's).
    pub rng: u32,
    /// Its updates so far.
    pub updates: u32,
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
}

/// The random generator's seed of a shown projectile: one fixed seed per
/// projectile of the shot (the server's depends on the weapon's own
/// generator, which the shot message does not carry).
pub fn shown_seed(i: usize) -> u32 {
    0x5EED_5409 ^ (i as u32).wrapping_mul(0x9E37_79B9)
}

/// The kick left on a shooter: its rotation (rad) and ticks.
#[derive(Component, Clone, Copy, Debug)]
pub struct HaloKick {
    pub rotation: f32,
    pub ticks: u32,
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

/// A gun's effect at its markers: the first person's for `first`, the
/// third person's for everyone.
pub fn gun_instances(fx: &HaloFx, effect: usize, gun: &FxGun, camera: Frame) -> Vec<Vec<Instance>> {
    fx.effects[effect]
        .locations
        .iter()
        .map(|l| {
            let name = l.to_ascii_lowercase();
            vec![Instance { first: place::first_person_frame(camera, gun, &name), third: place::third_person_frame(camera, gun, &name) }]
        })
        .collect()
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
    mut commands: Commands,
) {
    let authority = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    for f in fired.read() {
        let Some(gun) = guns.0.get(f.id) else { continue };
        for (i, (dir, tracer)) in f.projectiles.iter().enumerate() {
            commands.spawn((
                HaloShown {
                    gun: gun.clone(),
                    id: f.id,
                    owner: f.owner,
                    flight: super::super::weapons::flight::Flight::new(&gun.projectile, f.origin / METERS_PER_UNIT, *dir, 0.0),
                    ignored: None,
                    tracer: *tracer,
                    rng: shown_seed(i),
                    updates: 0,
                },
                Name::new("halo_ce shown projectile"),
            ));
        }
        let Some(lib) = fx.0.clone() else { continue };
        let Some(look) = lib.gun(&gun.tag) else { continue };
        let view = holders.get(f.owner).map_or(Quat::IDENTITY, |i| chimera_sim::weapon::aim_of(i.yaw, i.pitch));
        let camera = place::camera(f.origin, view);
        let fp = first_person(f.owner, &locals, role.as_deref());
        let list = if f.misfired { &look.misfire } else { &look.firing };
        if let Some(effect) = list.get(f.effect as usize).copied().flatten() {
            let inst = gun_instances(&lib, effect, look, camera);
            start(&mut effects, &lib, effect, inst, |r| {
                r.first_person = fp;
                r.owner = Some(f.owner);
                r.decals = authority;
            });
        }
        if !f.misfired && let Some((light, marker)) = &look.light {
            let at = place::first_person_frame(camera, look, marker).at;
            lights.write(HaloLight {
                at,
                color: Vec3::from(light.color),
                radius: light.radius * METERS_PER_UNIT,
                seconds: muzzle_light_ticks(gun) / 30.0,
                owner: Some(f.owner),
            });
        }
        if fp && let Some((rotation, ticks)) = look.kick {
            commands.entity(f.owner).insert(HaloKick { rotation, ticks });
        }
    }
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
        if !RecoilGame::is(game, GAME) {
            commands.entity(e).remove::<HaloKick>();
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
            commands.entity(e).remove::<HaloKick>();
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
    mut shown: Query<(Entity, &mut HaloShown)>,
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
    for (e, mut s) in &mut shown {
        for _ in 0..n {
            let s = &mut *s;
            let gun = s.gun.clone();
            let proj = &gun.projectile;
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
                Some(Hit {
                    t,
                    point,
                    normal,
                    object: what.map(|w| w.0),
                    material: if what.is_some() { Some(CYBORG as u8) } else { material },
                    unit: what.is_some(),
                })
            };
            // The server's rule without its damage roll: a projectile that
            // can hurt meets the shield or the body by the shield's state.
            let hurts = proj.impact_effect().is_some();
            let mut struck = |h: &Hit<Entity>, _: Vec3, _: f32| -> Option<u8> {
                let target = h.object?;
                hurts.then(|| if trace.shielded(target) { CYBORG_SHIELD as u8 } else { CYBORG as u8 })
            };
            let mut ignored = s.ignored;
            let mut rng = HaloRng(s.rng);
            let moved = s.flight.update(proj, &mut ignored, &mut rng, None, &mut tracer, &mut struck);
            s.rng = rng.0;
            s.ignored = ignored;
            s.updates += 1;
            if let Some(lib) = &lib {
                let look = lib.gun(&gun.tag);
                for (from, to, hit) in &moved.segments {
                    let (a, b) = (*from * METERS_PER_UNIT, *to * METERS_PER_UNIT);
                    if let Some(look) = look {
                        draw_segment(&mut particles, look, a, b, s.tracer, s.updates == 1);
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
                    let Some(effect) = look.and_then(|l| l.impacts.get(material).copied().flatten()) else { continue };
                    let frames = impact_frames(b, h.normal, b - a);
                    let inst = instances_from(lib, effect, &frames, frames["normal"]);
                    start(&mut effects, lib, effect, inst, |r| {
                        r.owner = Some(owner);
                        r.decals = authority && h.object.is_none();
                    });
                }
            }
            if matches!(moved.end, Some(super::super::weapons::flight::End::Detonate | super::super::weapons::flight::End::Disappear)) {
                commands.entity(e).despawn();
                break;
            }
        }
    }
}

/// A tick's segment of a shown projectile: the tracer's streak (its
/// contrail's width and colour over the segment, gone by the next tick)
/// or, for a projectile with a light (the plasma bolt), its glow at the
/// segment's end and its trail.
pub fn draw_segment(particles: &mut Particles, look: &FxGun, a: Vec3, b: Vec3, tracer: bool, first: bool) {
    let Some(t) = &look.tracer else { return };
    let Some(material) = t.material else { return };
    let bolt = look.bolt_light.is_some();
    if !tracer && !bolt {
        return;
    }
    let _ = first;
    let width = t.width * METERS_PER_UNIT;
    let color = Vec3::new(t.color[0], t.color[1], t.color[2]);
    let mut g = ParticleGroup::new(Default::default());
    g.capped = false;
    g.kill_after = Some(1.0 / 30.0 + 0.004);
    let mut streak = Particle {
        color,
        alpha: t.color[3].max(0.5),
        fade: Fade::Ramp(Ramp::constant(1.0)),
        shape: Shape::Ribbon { to: b, width: [width * 0.25, width], u: (0.0, 1.0), end_color: None, prev: None, next: None },
        ..Particle::new(a, 1.0 / 30.0, material, width)
    };
    if bolt {
        let light = look.bolt_light.as_ref().map_or(color, |l| Vec3::from(l.color));
        streak.color = light;
        let glow = Particle { color: light, ..Particle::new(b, 1.0 / 30.0, material, (width * 2.0).max(0.12)) };
        g.particles.push(glow);
    }
    g.particles.push(streak);
    particles.add(g);
}

/// Step the running effects a Halo tick and write what they made.
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
) {
    let Some(lib) = fx.0.clone() else { return };
    let world = level_trace(&spatial, &characters);
    let mut made = Made::default();
    let n = halo_ticks(&clock);
    let Effects { running, rng } = &mut *effects;
    for r in running.iter_mut() {
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
        let eye = at.translation + ms.map_or(Vec3::ZERO, |m| m.eye_offset);
        let camera = place::camera(eye, chimera_sim::weapon::aim_of(intent.yaw, intent.pitch));
        let fp = first_person(ev.owner, &locals, role.as_deref());
        let effect = match &ev.kind {
            WeaponEventKind::DryFire => look.empty.first().copied().flatten(),
            WeaponEventKind::Action("overheated") => look.overheated,
            WeaponEventKind::Deployed => look.ready,
            _ => None,
        };
        if let Some(effect) = effect {
            let inst = gun_instances(&lib, effect, look, camera);
            start(&mut effects, &lib, effect, inst, |r| {
                r.first_person = fp;
                r.owner = Some(ev.owner);
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
            trigger: 0,
            misfired: s.misfired,
            last_round: s.last_round,
            effect: s.effect,
            origin: Vec3::from(s.origin),
            projectiles: s.projectiles.iter().map(|(d, t)| (Vec3::from(*d), *t)).collect(),
            heat: 0.0,
            age: 0.0,
        });
    }
}

fn register_net(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.add_server_message::<HaloShotNet>(Channel::Unreliable)
        .add_systems(PostUpdate, send_shots.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, receive_shots.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)));
}

pub(super) fn plugin(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    chimera_sim::weapon::impact::ImpactAppExt::register_impact_provider(app, GAME);
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
