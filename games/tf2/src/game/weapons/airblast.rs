//! The Pyro's airblast (specs/tf2/weapons_more.md 7.4): attack2 with a
//! Flame Thrower whose `tf2:flame` names an `airblast` (`AirblastParams`),
//! or a weapon with its own `tf2:airblast` (the Dragon's Fury,
//! batch3b_mechanics.md 5); either way the part `Tf2Airblast` (its numbers
//! and the next airblast's time, predicted). An item's `mult_airblast_cost`
//! scales its cost and `airblast_disabled` takes it away (`items::docs`);
//! a teammate put out heals the Pyro `extinguish_heal` (20); a target's
//! items' `airblast_taken` (the Mantreads' 0.25) scales the power he
//! takes; the Cow Mangler's shots aren't turned (`no_reflect`).
//!
//! - **Firing** (`fire`, predicted, before the frame): refused while the
//!   next airblast's time hasn't come, with the eye under water or fewer
//!   than `cost` (20) in the clip; else, that tick, attack is dropped (the
//!   stream stops: `flame::sounds` ends the burst), `cost` leaves the
//!   clip, the next airblast is `refire` (0.75 s) and the next flame
//!   `flame_after` (1.0 s) later, the arms play `AIRBLAST` (`ft_alt_fire`,
//!   `view`), the sound (`sound`) plays on the Pyro, and the server's
//!   deflection (`Airblast`) and the look (`AirblastLook`) are sent.
//! - **What it reaches** (`deflect`, the server's, `FlyerSet::Think`): a
//!   sphere of `reach` (128) units centred `reach` ahead of the eye along
//!   the aim, each thing in it seen from the eye (a line not blocked by
//!   the world); an enemy player's box must also meet a cone of `cone`
//!   (35) degrees about the aim, 2 x `reach` long from the eye
//!   (`box_meets_cone`). The Pyro, the dead and spectators are skipped.
//! - **Teammates** in the sphere: a burning one is put out (`Burning`
//!   gone, `flame_out_sound` on him); never pushed.
//! - **An enemy player** is pushed (`push_velocity`): along the aim's
//!   pitch at the yaw from the Pyro's centre to his (`push_dir`), his
//!   speed toward the Pyro mirrored (`reflect` 2) and `power` (600) less
//!   half the mirror added; standing, bounced off the ground under him
//!   (a `ground_probe` (5) unit sweep of his box) and lifted at least
//!   `ground_min_z` (100) up at constant length. The velocity is added
//!   (Chimera's `ExternalPush`, so any game's body honours it: lifted,
//!   the ground limit after `no_footing` (0.5 s) without footing, sliding
//!   at `slide_friction` (0.1) of the friction at `slide_speed` (50) or
//!   more, air acceleration x `air_control` x `air_friction` (0.25 x
//!   0.75) until he lands), his view punched a whole 10 to 15 degrees.
//! - **Enemy projectiles** in the sphere (the rockets, pipes and stickies
//!   of `RocketRules`, `GrenadeRules`, `StickyRules`; not needles): their
//!   speed kept, turned toward where the eye's aim first meets the world
//!   (players aside, `TRACE_LENGTH`); rockets and pipes become the Pyro's
//!   (owner: damage credit, team) and mini-crit what they hit
//!   (`Deflected`, `blast::MINI_CRIT`); stickies stay their owner's, a stuck
//!   one knocked loose at `sticky_push` (2000) from the Pyro's centre out
//!   and not sticking again for `no_restick` (1 s, `sticky::Unstuck`).
//!   Each plays `deflect_sound` and shows the gun's `deflect` look
//!   (`deflect_fx`, `looks`); a push plays `impact_sound` at the Pyro.
//!
//! Known shortcuts: no lag compensation (players are tested where the
//! server has them); a crit-boosted Pyro's reflected rockets aren't
//! critical (no blast crits); the third-person body plays no gesture; the
//! air current's slide along walls isn't modelled (Open questions 25);
//! the view punch is the Source pitch down; the cone test is the box's
//! point nearest the cone's axis (iterated), not an exact box-cone test;
//! a reflected pipe's trail keeps its first team colour.

use bevy::prelude::*;
use serde::Deserialize;

use avian3d::prelude::ColliderAabb;
use chimera_sim::weapon::{
    Inventory, Magazine, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState, aim_of,
    projectile::Flyer,
    random::shared_random,
};

use super::{
    blast::{GROUND_PUSH_LIMIT, boxed},
    damage::U,
    flame::{Burning, FlameWorld},
    grenade::GrenadeRules,
    rocket::RocketRules,
    sticky::{StickyRules, StuckNormal, Unstuck},
};
use crate::{
    core::{ExternalPush, FirstTimePredicted, Health, Intent, MovementState, SimClock, Spectating, Team, Velocity},
    map::{GameSound, PlaySound, PredictedSound},
};

/// The view-model action of an airblast (`ft_alt_fire`).
pub const AIRBLAST: &str = "airblast";

/// Source's longest trace (units): where the aim point's line ends.
pub const TRACE_LENGTH: f32 = 56756.0;

/// The airblast's numbers (`[hooks."tf2:flame".airblast]`; units,
/// seconds, degrees, units/s).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AirblastParams {
    pub cost: u32,
    pub refire: f32,
    pub flame_after: f32,
    pub reach: f32,
    pub cone: f32,
    pub power: f32,
    pub reflect: f32,
    pub cost_coeff: f32,
    pub ground_min_z: f32,
    pub ground_probe: f32,
    pub punch: [f32; 2],
    pub no_footing: f32,
    pub slide_speed: f32,
    pub slide_friction: f32,
    pub air_control: f32,
    pub air_friction: f32,
    pub sticky_push: f32,
    pub no_restick: f32,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default)]
    pub deflect_sound: Option<String>,
    #[serde(default)]
    pub impact_sound: Option<String>,
    #[serde(default)]
    pub flame_out_sound: Option<String>,
    /// Health its Pyro gains for each burning teammate it puts out
    /// (`extinguish_restores_health`, batch3b_mechanics.md 5).
    #[serde(default)]
    pub extinguish_heal: f32,
}

/// The hook of an airblast on a weapon that isn't a Flame Thrower (the
/// Dragon's Fury, batch3b_mechanics.md 5; a Flame Thrower's is
/// `[hooks."tf2:flame".airblast]`).
pub const AIRBLAST_HOOK: &str = "tf2:airblast";

/// A weapon's airblast (a predicted part): its numbers and when the next
/// may go.
#[derive(Component, Clone, Debug)]
pub struct Tf2Airblast {
    pub params: AirblastParams,
    pub next_blast: f64,
}

impl chimera_sim::weapon::NetPart for Tf2Airblast {
    type Net = f64;
    fn net(&self) -> f64 {
        self.next_blast
    }
    fn set_net(&mut self, net: f64) {
        self.next_blast = net;
    }
}

/// An airblast on a weapon (`tf2:flame`'s or `tf2:airblast`).
pub fn attach(e: &mut EntityWorldMut, p: &AirblastParams) {
    e.insert(Tf2Airblast { params: p.clone(), next_blast: 0.0 });
}

/// An airblast for the server to deflect with (`deflect`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Airblast {
    pub pyro: Entity,
    pub weapon: Entity,
    /// The eye and the aim (unit), engine space.
    pub eye: Vec3,
    pub aim: Vec3,
    /// The command it was made in (the view punch's seed).
    pub command: u32,
}

/// An airblast's look to show (`looks`): the Pyro's `pyro_blast`
/// (`deflect` false) or a deflected projectile's `deflect_fx`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct AirblastLook {
    pub entity: Entity,
    pub deflect: bool,
}

/// On a deflected projectile: how many times (rockets and pipes: they
/// mini-crit what they hit).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Deflected(pub u32);

/// The push direction (unit, engine space): the aim's pitch at the yaw of
/// the line from the Pyro's centre `from` to the target's `to`.
pub fn push_dir(from: Vec3, to: Vec3, aim: Vec3) -> Vec3 {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z).try_normalize().unwrap_or_else(|| Vec3::new(aim.x, 0.0, aim.z).normalize_or(Vec3::NEG_Z));
    let pitch = aim.y.clamp(-1.0, 1.0).asin();
    flat * pitch.cos() + Vec3::Y * pitch.sin()
}

/// The velocity (units/s, engine space: y up) added to a target moving
/// at `v` pushed along `d` (unit); `ground`: the normal of the ground
/// he stands on (None: in the air).
pub fn push_velocity(v: Vec3, d: Vec3, p: &AirblastParams, ground: Option<Vec3>) -> Vec3 {
    let m = v.dot(d);
    let mut r = if m < 0.0 { -p.reflect * m * d } else { Vec3::ZERO };
    r += (p.power - p.cost_coeff * r.length()).max(0.0) * d;
    let Some(n) = ground else { return r };
    // Bounced off the ground rather than into it.
    let h = (v + r).dot(n);
    if h < 0.0 {
        r += -2.0 * h * n;
    }
    // Lifted at least `ground_min_z` at constant length.
    let mut need = p.ground_min_z - (v + r).y;
    if need > 0.0 {
        let x = Vec2::new(r.x, r.z).length();
        need = need.min(x);
        if r.length() < need {
            r = Vec3::new(0.0, r.length(), 0.0);
        } else if x > 0.0 {
            let s = (x * x - need * need - 2.0 * need * r.y).max(0.0).sqrt() / x;
            r = Vec3::new(s * r.x, r.y + need, s * r.z);
        }
    }
    r
}

/// Whether the box `min`..`max` meets the sphere of radius `r` at `c`.
pub fn box_meets_sphere(c: Vec3, r: f32, min: Vec3, max: Vec3) -> bool {
    c.clamp(min, max).distance(c) <= r
}

/// Whether the box `min`..`max` meets the cone from `apex` along `axis`
/// (unit), `half_angle` (radians) wide and `length` long: the box's point
/// nearest the axis segment (found by turns) inside it.
pub fn box_meets_cone(apex: Vec3, axis: Vec3, half_angle: f32, length: f32, min: Vec3, max: Vec3) -> bool {
    if apex.clamp(min, max) == apex {
        return true;
    }
    let tan = half_angle.tan();
    let mut p = (min + max) / 2.0;
    for _ in 0..4 {
        let t = (p - apex).dot(axis).clamp(0.0, length);
        p = (apex + axis * t).clamp(min, max);
    }
    let along = (p - apex).dot(axis);
    if !(0.0..=length).contains(&along) {
        return false;
    }
    (p - apex - axis * along).length() <= along * tan + 1e-4
}

/// Before the frame (predicted): attack2 with a drawn Flame Thrower that
/// can airblast (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn fire(
    mut owners: Query<(Entity, &Inventory, &mut Intent, &Transform, Option<&MovementState>, Option<&Health>)>,
    mut guns: Query<(&mut Tf2Airblast, &mut WeaponState, Option<&mut Magazine>)>,
    world: FlameWorld,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    mut events: MessageWriter<WeaponEvent>,
    mut blasts: MessageWriter<Airblast>,
    mut looks: MessageWriter<AirblastLook>,
    mut sounds: MessageWriter<PredictedSound>,
) {
    let now = clock.now;
    for (owner, inv, mut intent, at, state, health) in &mut owners {
        if !intent.secondary || health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(w) = inv.active else { continue };
        let Ok((mut f, mut st, mag)) = guns.get_mut(w) else { continue };
        let p = f.params.clone();
        if f.next_blast > now + 1e-5 {
            continue;
        }
        let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        if world.wet(eye) {
            continue;
        }
        let Some(mut mag) = mag.filter(|m| m.clip >= p.cost) else { continue };
        mag.clip -= p.cost;
        f.next_blast = now + p.refire as f64;
        st.next_primary = st.next_primary.max(now + p.flame_after as f64);
        // The airblast wins the tick: no flame (the burst ends here).
        intent.fire = false;
        events.write(WeaponEvent { owner, weapon: w, kind: WeaponEventKind::Action(AIRBLAST), replay: !first.0 });
        if !first.0 {
            continue;
        }
        let aim = aim_of(intent.yaw, intent.pitch) * Vec3::NEG_Z;
        blasts.write(Airblast { pyro: owner, weapon: w, eye, aim, command: intent.command });
        looks.write(AirblastLook { entity: owner, deflect: false });
        if let Some(s) = p.sound {
            sounds.write(PredictedSound {
                by: owner,
                sound: PlaySound { entry: s, at: Some(at.translation), volume: None, pitch: None, source: Some(owner), channel: None },
            });
        }
    }
}

/// A character's parts the deflection reads and writes.
type Chars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static ColliderAabb>,
        &'static Health,
        Option<&'static Team>,
        Option<&'static mut MovementState>,
        &'static mut Velocity,
        Option<&'static mut ExternalPush>,
        Option<&'static mut ViewPunch>,
        Has<Burning>,
        Has<Spectating>,
    ),
    With<Intent>,
>;

/// What a deflected projectile is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Rocket,
    Pipe,
    Sticky,
}

/// The server: each airblast's pushes, put-out teammates and deflected
/// projectiles (module doc).
#[allow(clippy::too_many_arguments)]
pub fn deflect(
    mut blasts: MessageReader<Airblast>,
    guns: Query<&Tf2Airblast>,
    mut chars: Chars,
    (holders, items, rules): (Query<&Inventory>, Query<&crate::game::items::mods::Tf2Item>, Option<Res<crate::game::items::effects::ItemRules>>),
    mut flyers: Query<(Entity, &mut Flyer, &Transform, Option<&mut Deflected>)>,
    (rockets, grenades, stickies): (Res<RocketRules>, Res<GrenadeRules>, Res<StickyRules>),
    world: FlameWorld,
    clock: Res<SimClock>,
    mut sounds: MessageWriter<GameSound>,
    mut looks: MessageWriter<AirblastLook>,
    mut commands: Commands,
) {
    let blasts: Vec<Airblast> = blasts.read().copied().collect();
    for b in blasts {
        let Some(p) = guns.get(b.weapon).ok().map(|f| f.params.clone()) else { continue };
        let Ok((_, at, aabb, _, team, state, ..)) = chars.get(b.pyro) else { continue };
        let (lo, hi) = boxed(at, aabb, state.as_deref());
        let pyro_centre = (lo + hi) / 2.0;
        let pyro_team = team.copied().filter(|t| t.0 != 0);
        let mates = |t: Option<&Team>| pyro_team.is_some() && t.copied() == pyro_team;
        let (r, centre) = (p.reach * U, b.eye + b.aim * p.reach * U);
        let mut pushed = false;
        for (e, at, aabb, health, team, mut state, mut velocity, push, punch, burning, spectating) in &mut chars {
            if e == b.pyro || health.current <= 0.0 || spectating {
                continue;
            }
            let (min, max) = boxed(at, aabb, state.as_deref());
            if !box_meets_sphere(centre, r, min, max) {
                continue;
            }
            let c = (min + max) / 2.0;
            if world.line_blocked(b.eye, c) {
                continue;
            }
            if mates(team) {
                // A burning teammate is put out; none is pushed.
                if burning {
                    commands.entity(e).remove::<Burning>();
                    if let Some(s) = &p.flame_out_sound {
                        sounds.write(GameSound(PlaySound { source: Some(e), ..PlaySound::at(s.clone(), c) }));
                    }
                    if p.extinguish_heal > 0.0 {
                        heal(&mut commands, b.pyro, p.extinguish_heal);
                    }
                }
                continue;
            }
            if !box_meets_cone(b.eye, b.aim, p.cone.to_radians(), 2.0 * r, min, max) {
                continue;
            }
            let d = push_dir(pyro_centre, c, b.aim);
            let on_ground = state.as_ref().is_some_and(|s| s.on_ground);
            let ground = on_ground.then(|| world.ground_under(min, max, p.ground_probe * U).unwrap_or(Vec3::Y));
            // His items' airblast factor (the Mantreads' 0.25 on the
            // power, before the ground lift: batch3b_mechanics.md 9.2).
            let taken = crate::game::items::kit::airblast_taken(holders.get(e).ok(), &items);
            let scaled;
            let p_used = if (taken - 1.0).abs() > 1e-6 {
                scaled = AirblastParams { power: p.power * taken, ..p.clone() };
                &scaled
            } else {
                &p
            };
            let add = push_velocity(velocity.0 / U, d, p_used, ground);
            velocity.0 += add * U;
            // Off his feet (every movement honours it: `ExternalPush`).
            if let Some(mut push) = push {
                *push = ExternalPush {
                    ground_limit: GROUND_PUSH_LIMIT,
                    lift: true,
                    no_footing: p.no_footing,
                    slide_min: p.slide_speed * U,
                    slide_friction: p.slide_friction,
                    air_control: p.air_control * p.air_friction,
                };
            }
            if let Some(s) = state.as_mut().filter(|s| s.on_ground) {
                s.on_ground = false;
            }
            // A whole 10 to 15 degrees, Source's pitch down.
            let roll = shared_random(b.command, "tf2:airblast_punch", e.index_u32() as i32).next();
            let span = p.punch[1] - p.punch[0];
            let degrees = (p.punch[0] + (roll * (span + 1.0)).floor()).min(p.punch[1]);
            match punch {
                Some(mut v) => v.0.x -= degrees.to_radians(),
                None => {
                    commands.entity(e).insert(ViewPunch(Vec3::new(-degrees.to_radians(), 0.0, 0.0)));
                }
            }
            pushed = true;
        }
        if pushed && let Some(s) = &p.impact_sound {
            sounds.write(GameSound(PlaySound { source: Some(b.pyro), ..PlaySound::at(s.clone(), b.eye) }));
        }
        // Where the aim first meets the world (players aside).
        let aim_point = b.eye + b.aim * world.line_reach(b.eye, b.aim, TRACE_LENGTH * U);
        for (e, mut f, t, deflected) in &mut flyers {
            let kind = if rockets.get(f.weapon).is_some() {
                Kind::Rocket
            } else if grenades.0.get(f.weapon).is_some() {
                Kind::Pipe
            } else if stickies.get(f.weapon).is_some() {
                Kind::Sticky
            } else {
                continue;
            };
            // Not the Cow Mangler's (batch3b_mechanics.md 11).
            if rules.as_deref().and_then(|r| r.get(f.weapon)).is_some_and(|p| p.no_reflect) {
                continue;
            }
            // Enemy projectiles only (no team: anyone's but his own).
            if f.owner == Some(b.pyro) {
                continue;
            }
            let owner_team = f.owner.and_then(|o| chars.get(o).ok()).and_then(|c| c.4.copied());
            if mates(owner_team.as_ref()) {
                continue;
            }
            let at = t.translation;
            if at.distance(centre) > r || world.line_blocked(b.eye, at) {
                continue;
            }
            if f.stuck_until.is_some() {
                // A stuck sticky: knocked loose out from the Pyro.
                let u = (at - pyro_centre).normalize_or(b.aim);
                f.velocity = u * p.sticky_push * U;
                f.stuck_until = None;
                commands.entity(e).remove::<StuckNormal>().insert(Unstuck(clock.now + p.no_restick as f64));
            } else {
                let u = (aim_point - at).normalize_or(b.aim);
                f.velocity = u * f.velocity.length();
                if kind == Kind::Sticky {
                    commands.entity(e).insert(Unstuck(clock.now + p.no_restick as f64));
                }
            }
            if kind != Kind::Sticky {
                // The Pyro's now: his credit, his team; mini-crits.
                f.owner = Some(b.pyro);
                match deflected {
                    Some(mut d) => d.0 += 1,
                    None => {
                        commands.entity(e).insert(Deflected(1));
                    }
                }
            }
            if let Some(s) = &p.deflect_sound {
                sounds.write(GameSound(PlaySound { source: Some(e), ..PlaySound::at(s.clone(), at) }));
            }
            looks.write(AirblastLook { entity: e, deflect: true });
        }
    }
}

/// `pyro` healed `points` up to his max (an extinguish's).
pub fn heal(commands: &mut Commands, pyro: Entity, points: f32) {
    commands.entity(pyro).queue(move |mut w: EntityWorldMut| {
        if let Some(mut h) = w.get_mut::<Health>()
            && h.current > 0.0
            && h.current < h.max
        {
            h.current = (h.current + points / 100.0).min(h.max);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> AirblastParams {
        super::super::flame::stock_params().airblast.expect("the stock Flame Thrower airblasts")
    }

    /// weapons_more.md 7.4 (measured): a standing target at level aim
    /// takes 591.6 along the push and 100 up (600 lifted at constant
    /// length).
    #[test]
    fn a_standing_target_at_level_aim_takes_591_6_and_100_up() {
        let p = params();
        let d = push_dir(Vec3::ZERO, Vec3::new(0.0, 0.0, -3.0), Vec3::NEG_Z);
        let r = push_velocity(Vec3::ZERO, d, &p, Some(Vec3::Y));
        assert!((Vec2::new(r.x, r.z).length() - 591.6).abs() < 0.05, "{r}");
        assert!((r.y - 100.0).abs() < 1e-3, "{r}");
        // In the air: 600 along the push, nothing more.
        let air = push_velocity(Vec3::ZERO, d, &p, None);
        assert!((air.length() - 600.0).abs() < 1e-3 && air.y.abs() < 1e-3, "{air}");
    }

    /// A target running at the Pyro has that speed mirrored, and only the
    /// power left after half the mirror is added.
    #[test]
    fn a_target_coming_at_the_pyro_is_mirrored() {
        let p = params();
        let d = Vec3::NEG_Z;
        let r = push_velocity(Vec3::Z * 300.0, d, &p, None);
        // 600 mirrored, 600 - 300 = 300 added: 900 along d.
        assert!((r - d * 900.0).length() < 1e-3, "{r}");
        // Aiming down at a standing target bounces him up.
        let down = push_dir(Vec3::ZERO, Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, -0.7071, -0.7071));
        let r = push_velocity(Vec3::ZERO, down, &p, Some(Vec3::Y));
        assert!(r.y > 100.0, "bounced up: {r}");
    }

    /// The pitch is the aim's, the yaw the line's to the target.
    #[test]
    fn the_push_takes_the_aims_pitch_and_the_targets_yaw() {
        let aim = Vec3::new(0.0, 0.5f32.sin(), -0.5f32.cos());
        let d = push_dir(Vec3::ZERO, Vec3::new(3.0, 1.0, 0.0), aim);
        assert!((d - Vec3::new(0.5f32.cos(), 0.5f32.sin(), 0.0)).length() < 1e-5, "{d}");
    }

    #[test]
    fn boxes_meet_the_cone_within_35_degrees() {
        let cone = 35f32.to_radians();
        let b = |c: Vec3| (c - Vec3::splat(0.5), c + Vec3::splat(0.5));
        let (lo, hi) = b(Vec3::new(0.0, 0.0, -3.0));
        assert!(box_meets_cone(Vec3::ZERO, Vec3::NEG_Z, cone, 6.5, lo, hi));
        // Off to the side at 60 degrees: out.
        let (lo, hi) = b(Vec3::new(3.0 * 60f32.to_radians().sin(), 0.0, -3.0 * 60f32.to_radians().cos()) * 1.5);
        assert!(!box_meets_cone(Vec3::ZERO, Vec3::NEG_Z, cone, 6.5, lo, hi));
        // Behind: out; beyond the length: out.
        let (lo, hi) = b(Vec3::new(0.0, 0.0, 3.0));
        assert!(!box_meets_cone(Vec3::ZERO, Vec3::NEG_Z, cone, 6.5, lo, hi));
        let (lo, hi) = b(Vec3::new(0.0, 0.0, -8.0));
        assert!(!box_meets_cone(Vec3::ZERO, Vec3::NEG_Z, cone, 6.5, lo, hi));
        assert!(box_meets_sphere(Vec3::ZERO, 1.0, Vec3::new(0.5, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0)));
        assert!(!box_meets_sphere(Vec3::ZERO, 1.0, Vec3::new(1.5, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0)));
    }
}
