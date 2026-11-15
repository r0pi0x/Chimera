//! Halo's weapons in the engine's tick (`super::HaloWeaponsPlugin`): the
//! holders' 30 Hz updates in the spec's per-tick order (inputs, a switch's
//! put-away and ready, the player melee, then every carried weapon's
//! `HaloGun::tick`), the shots' projectiles (created and first moved on
//! the firing tick, on the server), the flight of the ones already out,
//! their hits through the damage seam (`HaloHit::direct`, `HaloHit::melee`)
//! and what presentation reads (`WeaponEvent`s, `HaloFired`, `HaloImpact`,
//! the zoom as `Zoomed`).
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
//! moved onto the camera ray, has no model yet to come from; the four have
//! no first-person offset); a fired projectile's flight is the server's
//! (the shooter's client predicts the weapon, ammo, heat and the shot's
//! `HaloFired`; every machine shows the projectile by flying it again from
//! the shot, `fx::shots`, the server sending others' shots); later updates of
//! a projectile meet characters where they are now (lag compensation
//! rewinds the firing tick's update only); the hit's "shield or body"
//! material is the target's shield above 0 before the hit (the seam knows
//! the exact share only after); the level's materials are not known, so
//! every level surface is the projectiles' `on_level` (disappear: glass,
//! leaves and water do not pass); dead bodies are not hit; the melee's
//! 25 probes meet characters only (no breakable surfaces, no vehicles),
//! and its response damage on the attacker is not applied (all zero on
//! the disc).

use std::sync::Arc;

use avian3d::prelude::{ColliderAabb, ColliderOf, SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemParam, prelude::*};
use chimera_sim::weapon::{
    Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, Zoomed, aim_of, hitgroup_at,
    lagcomp::{LagComp, Rewound},
    projectile::ray_box,
};
use serde::{Deserialize, Serialize};

use super::{
    METERS_PER_WU,
    flight::{End, Flight, Hit},
    params::{Gun, Response, effect_id, weapon_flags},
    random::HaloRng,
    tick::{GunEvent, HaloGun, Inputs},
};
use crate::{
    core::{
        ActionRegistry, BodyRegion, FirstTimePredicted, Health, HitShapes, Intent, MovementState, NetRole, SimClock,
        Velocity,
    },
    map::interp::NetDrawn,
};

use super::super::{
    damage::{HaloHit, HaloHurt},
    vitality::{DamageEffects, Spartan, Vitality, melee_scale},
};

/// The named actions Halo's weapons read beside fire, attack2 and reload.
pub const MELEE_ACTION: &str = "melee";
pub const ZOOM_ACTION: &str = "zoom";

/// A Halo weapon's numbers (its data's `halo_ce:weapon` table).
#[derive(Component, Clone, Debug)]
pub struct HaloWeapon(pub Arc<Gun>);

/// The 30 Hz ticks whose boundary this engine tick crossed (module doc):
/// 1 at Halo's own tick.
pub fn halo_ticks(clock: &SimClock) -> u32 {
    super::super::ticks::halo_ticks(clock)
}

/// A holder's own part of Halo's weapon rules: the player melee's
/// counter, the ticks airborne (the melee's lunge scale), the inputs seen
/// since the last Halo tick (a press between two is not lost).
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
}

/// One round fired by a Halo weapon (for presentation: the first-person
/// fire animation, the muzzle effect by entry, the sound, a tracer): on
/// the shooter's tick, predicted, not on a replay.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloFired {
    pub owner: Entity,
    pub weapon: Entity,
    pub id: &'static str,
    /// The trigger (0: the primary; the four have one).
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
}

/// A projectile's segment of one update (server): from, to (metres), what
/// it met there (None: nothing; `Some(None)`: the level), the normal.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloImpact {
    pub owner: Entity,
    pub id: &'static str,
    pub from: Vec3,
    pub to: Vec3,
    pub hit: Option<Option<Entity>>,
    pub normal: Vec3,
    pub tracer: bool,
}

/// A projectile in flight (server).
#[derive(Component, Clone, Debug)]
pub struct HaloProjectile {
    pub gun: Arc<Gun>,
    pub id: &'static str,
    pub owner: Entity,
    pub flight: Flight,
    pub ignored: Option<Entity>,
    pub rng: u32,
    pub tracer: bool,
}

/// What a projectile or a melee probe traces: the level (avian) and the
/// characters' hit volumes (their hitboxes, else their box), the
/// shooter's view of them when lag compensation rewound them.
#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct HaloTrace<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
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
}

/// What a trace met (metres): the distance along it, the point, the
/// normal, and the character with the region hit (None: the level).
pub type Met = (f32, Vec3, Vec3, Option<(Entity, Option<BodyRegion>)>);

impl HaloTrace<'_, '_> {
    /// The nearest thing on `from` to `to` (metres), leaving out `skip`.
    pub fn trace(&self, from: Vec3, to: Vec3, skip: &[Entity], rewound: &[Rewound]) -> Option<Met> {
        let (dir, len) = Dir3::new_and_length(to - from).ok()?;
        let all: Vec<Entity> = self.characters.iter().map(|c| c.0).collect();
        let filter = SpatialQueryFilter::from_excluded_entities(all.iter().copied().chain(skip.iter().copied()))
            .with_mask(crate::core::SOLID_LAYERS);
        let colliders = &self.colliders;
        let chars = &self.characters;
        let mut best: Option<Met> = self
            .spatial
            .cast_ray_predicate(from, dir, len, true, &filter, &|c| {
                let body = colliders.get(c).map_or(c, |o| o.body);
                !chars.contains(body)
            })
            .map(|h| (h.distance, from + *dir * h.distance, h.normal, None));
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
                    best = Some((enter, from + *dir * enter, -*dir, Some((e, Some(boxes.0[i].region)))));
                }
            } else if let Some(b) = aabb
                && let Some(enter) = ray_box(b.min, b.max, from, *dir, lim)
            {
                let p = from + *dir * enter;
                best = Some((enter, p, -*dir, Some((e, Some(BodyRegion::from_hitgroup(hitgroup_at(b, p)))))));
            }
        }
        for r in rewound.iter().filter(|r| !skip.contains(&r.entity)) {
            let lim = limit(&best);
            if let Some(boxes) = r.hitboxes.as_ref().map(|h| h.get()) {
                let (feet, turn) = r.frame();
                if let Some((i, enter, _)) = boxes.nearest(turn * (from - feet), turn * *dir, lim) {
                    best = Some((enter, from + *dir * enter, -*dir, Some((r.entity, Some(boxes.0[i].region)))));
                }
            } else if let Some(enter) = r.box_entry(from, *dir, lim, 0.0) {
                let p = from + *dir * enter;
                best = Some((enter, p, -*dir, Some((r.entity, Some(BodyRegion::from_hitgroup(hitgroup_at(&r.aabb(), p)))))));
            }
        }
        best
    }

    /// Whether the shield takes a hit on `target` (a Halo character with
    /// shield left), else the body (projectiles.md 6, step 1).
    pub fn shielded(&self, target: Entity) -> bool {
        self.characters.get(target).ok().and_then(|c| c.6).is_some_and(|v| v.shield > 0.0)
    }
}

/// What a fired round's first update and later ones need on the server.
#[derive(SystemParam)]
pub struct Hits<'w> {
    hits: MessageWriter<'w, HaloHit>,
    impacts: MessageWriter<'w, HaloImpact>,
    effects: Res<'w, DamageEffects>,
}

/// One update of a projectile on the server: its flight, its direct hits
/// through the seam and its segments for presentation. True when it ended.
fn advance(p: &mut HaloProjectile, trace: &HaloTrace, rewound: &[Rewound], out: &mut Hits) -> bool {
    let gun = p.gun.clone();
    let proj = &gun.projectile;
    let owner = p.owner;
    let effect = effect_id(&proj.impact_damage);
    let mut rng = HaloRng(p.rng);
    let mut hits = Vec::new();
    // What a hit is: the character and the region hit (None: the level).
    type Struck = (Entity, Option<BodyRegion>);
    let moved = {
        let mut tracer = |a: Vec3, b: Vec3, ignored: Option<Struck>| -> Option<Hit<Struck>> {
            let skip: Vec<Entity> = std::iter::once(owner).chain(ignored.map(|i| i.0)).collect();
            let (_, point, normal, what) = trace.trace(a * METERS_PER_WU, b * METERS_PER_WU, &skip, rewound)?;
            let len = (b - a).length() * METERS_PER_WU;
            let t = if len > 0.0 { ((point - a * METERS_PER_WU).length() / len).clamp(0.0, 1.0) } else { 0.0 };
            Some(Hit { t, point: point / METERS_PER_WU, normal, object: what })
        };
        let mut struck = |h: &Hit<Struck>, v: Vec3, s: f32| -> Response {
            let Some((target, region)) = h.object else { return proj.on_level };
            let shield = trace.shielded(target);
            hits.push((target, region, h.point, v, s, rng.real()));
            if shield { proj.on_shield } else { proj.on_body }
        };
        let mut ignored = p.ignored.map(|e| (e, None));
        let m = p.flight.update(proj, &mut ignored, &mut tracer, &mut struck);
        p.ignored = ignored.map(|i| i.0);
        m
    };
    p.rng = rng.0;
    for (target, region, point, v, s, u) in hits {
        let Some(effect) = effect else { continue };
        let Some(fx) = out.effects.get(effect) else { continue };
        out.hits.write(
            HaloHit::direct(target, effect, fx.amount(s, u), s, region, point * METERS_PER_WU, v.normalize_or_zero(), Some(owner))
                .weapon(p.id),
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
        });
    }
    matches!(moved.end, Some(End::Detonate | End::Disappear))
}

/// The projectiles already out, one update per Halo tick (server; before
/// the weapons, which give the new ones their first update).
pub fn fly(
    mut projectiles: Query<(Entity, &mut HaloProjectile)>,
    trace: HaloTrace,
    mut out: Hits,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    for (e, mut p) in &mut projectiles {
        for _ in 0..n {
            if advance(&mut p, &trace, &[], &mut out) {
                commands.entity(e).despawn();
                break;
            }
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
    mut commands: Commands,
) {
    let n = halo_ticks(&clock);
    let server = role.as_deref().is_none_or(|r| *r != NetRole::Client);
    let bit = |name: &str| actions.as_deref().map_or(0, |a| a.bit(name));
    let (melee_bit, zoom_bit) = (bit(MELEE_ACTION), bit(ZOOM_ACTION));
    for (owner, intent, inv, at, state, vel, health, mut hands) in &mut holders {
        if !inv.weapons.iter().any(|w| guns.contains(*w)) {
            continue;
        }
        // The inputs seen since the last Halo tick.
        hands.fire |= intent.fire;
        hands.zoom |= intent.secondary || intent.actions & zoom_bit != 0;
        hands.melee_button |= intent.actions & melee_bit != 0;
        hands.reload |= intent.reload;
        if n == 0 {
            continue;
        }
        let alive = health.current > 0.0;
        let eye = at.translation + state.eye_offset;
        let aim = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        for _ in 0..n {
            // 2. The unit: a switch puts the old weapon away and readies
            // the new one at once.
            for &w in &inv.weapons {
                let Ok((weapon, hw, mut gun, _)) = guns.get_mut(w) else { continue };
                let held = inv.active == Some(w) && alive;
                if held != gun.in_hand {
                    let ev = if held { gun.ready(&hw.0) } else { gun.put_away(&hw.0) };
                    if let Some(GunEvent::RoundsIn { shell }) = ev {
                        event(&mut events, &first, owner, w, if shell { WeaponEventKind::ShellInserting } else { WeaponEventKind::Reloaded });
                    }
                    let _ = weapon;
                }
            }
            let busy = hands.melee > 0;
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
                let inputs = Inputs {
                    fire: held && hands.fire,
                    reload: held && hands.reload,
                    zoom: held && hands.zoom,
                    busy,
                    held,
                    switching: false,
                    aim,
                };
                let evs = gun.tick(&hw.0, &inputs);
                if let Some(mut m) = mag {
                    m.clip = gun.loaded.max(0) as u32;
                    m.reserve = gun.reserve.max(0) as u32;
                }
                for ev in evs {
                    match ev {
                        GunEvent::Fired { misfired, last_round, effect, projectiles } => {
                            let kind = if misfired { "misfire" } else if last_round { "fire_last" } else { "fire" };
                            if first.0 {
                                debug!("{} fired on tick {}: {} loaded, {} reserve, heat {}", weapon.id, clock.tick, gun.loaded, gun.reserve, gun.heat);
                            }
                            event(&mut events, &first, owner, w, WeaponEventKind::Action(kind));
                            if first.0 {
                                fired.write(HaloFired {
                                    owner,
                                    weapon: w,
                                    id: weapon.id,
                                    trigger: 0,
                                    misfired,
                                    last_round,
                                    effect,
                                    origin: eye,
                                    projectiles: projectiles.clone(),
                                    heat: gun.heat,
                                    age: gun.age,
                                });
                            }
                            if server && first.0 && !projectiles.is_empty() {
                                // Created and given their first update now
                                // (projectiles.md 4), against the others
                                // where the shooter saw them.
                                let rewound = lag.rewind(owner, true, None);
                                let inherited = vel.map_or(0.0, |v| v.0.dot(aim)) / METERS_PER_WU / 30.0;
                                for (i, (d, tracer)) in projectiles.into_iter().enumerate() {
                                    let mut p = HaloProjectile {
                                        gun: hw.0.clone(),
                                        id: weapon.id,
                                        owner,
                                        flight: Flight::new(&hw.0.projectile, eye / METERS_PER_WU, d, inherited),
                                        ignored: None,
                                        rng: gun.rng ^ (i as u32).wrapping_mul(0x9E37_79B9),
                                        tracer,
                                    };
                                    if !advance(&mut p, &trace, &rewound, &mut out) {
                                        commands.spawn((p, Name::new("halo_ce projectile")));
                                    }
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
                    }
                }
            }
        }
        hands.fire = false;
        hands.zoom = false;
        hands.melee_button = false;
        hands.reload = false;
        // The zoom as the camera's field of view (weapons.md 12).
        let zoomed = inv
            .active
            .and_then(|a| guns.get(a).ok())
            .filter(|(.., g, _)| g.zoom >= 0)
            .map(|(_, hw, g, _)| Zoomed {
                fov: (hw.0.camera_field_of_view / hw.0.magnification(g.zoom)).to_degrees(),
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
                    if let Some((d, _, _, Some((e, _)))) = trace.trace(eye, eye + probe, &[owner], &rewound)
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
    } else if hands.melee_button && !g.has(weapon_flags::PREVENTS_MELEE) && gun.zoom < 0 {
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
