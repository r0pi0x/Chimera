//! The machine guns' attack2s (specs/hl2dm/projectiles.md 4, 5): the
//! SMG1's contact grenade (`hl2dm:smg1_grenade`, `Hl2SmgGrenade`) and the
//! AR2's energy ball (`hl2dm:combine_ball`, `Hl2Ball`), each with its own
//! rounds (`launch::Hl2Rounds`: SMG1_Grenade 3, AR2AltFire 3; none at
//! spawn), predicted with the weapon; the projectiles are the server's
//! (Chimera's flyers, `weapon::projectile`).
//!
//! - **Contact grenade**: attack2 with the next secondary passed: with no
//!   grenade the empty sound (`Weapon_SMG1.Empty`, next secondary + 0.5,
//!   the frame goes on); with the eyes under water the generic frame's
//!   refusal first, every tick attack2 is held (weapons.md 5: the empty
//!   sound, next secondary + `hl2dm:dry_under_water`'s 0.2, the frame
//!   ended, no dry-fire animation; the AR2's alike); else 1 used, a
//!   running reload cancelled (no ammo moved), next primary + 0.5, next secondary + 1.0, the player not made
//!   busy; from the eye along view + punch at 1000 units/s (none of the
//!   shooter's), 400 units/s² of gravity, a ±3 box that explodes on the
//!   first solid thing it touches (passing loose weapons), 100 in 250
//!   (`launch::hl2_blast`).
//! - **Energy ball**: attack2 starts the charge (`Weapon_CombineGuard.
//!   Special1`; next primary and secondary + 0.5; the AR2 can't be put
//!   away: `weapon::HolsterLocked`); on the first tick strictly after
//!   0.5 s it launches (busy 0.7 s, next primary + 0.5, next secondary
//!   + 1.0, a punch of pitch -8..-12 and yaw 1..2 whole degrees): a
//!   10-unit sphere at 1000 units/s that keeps that speed after every
//!   contact (`Flyer::keep_speed`), no gravity, 4 s of life, never its
//!   owner, 1000 crush to every other player it touches (through the
//!   suit: 800 to health on 200 armour) bouncing on; after a world bounce
//!   at least 0.25 s since the last counted one it seeks (`seek`): the
//!   nearest attractive player within 15 degrees of its new direction in
//!   a 2048-unit sweep (±256), or once it has struck a player the nearest
//!   in front within 512. Its end at 4 s is harmless.
//!
//! The contact grenade's 3-tick check (4.2, `grenade_stopped`): every 3
//! ticks of flight one stopped dead explodes. The ball's seek (5.4):
//! attractive players are alive, not its owner, not the owner's teammates
//! in TDM (`Hl2Match::teamplay`, `core::Team`) and nobody in the
//! intermission (`Hl2Match::over`); before it has struck a player, those
//! whose bounds meet a box of half extent 256 swept 2048 along the new
//! direction (`seek`). The SMG1's grenade has no team test: it explodes on
//! any player but its owner (4.2; the spec names none). The shooter's
//! white fade at the launch is `projectile_fx`'s, the dissolve of the
//! ball's victims (the body and the dropped weapon) `dissolve`'s.
//!
//! Known shortcuts: the contact grenade's 3-tick check has no ground
//! entity to test (a flyer has none: any touch ends it), and its random
//! spin isn't given (a flyer faces its flight); the knock of what a
//! ball hits is a whole elastic hit of its 150 kg (the spec gives only that
//! it is knocked; the 750 kg after a gun isn't kept); its harmless end is
//! removed at once.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    HolsterLocked, Inventory, NetPart, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState,
    projectile::{Direct, Flyer, FlyerGone, FlyerHit, LaunchRequest, LaunchUnderWater, SkyEnd, Touch},
    random::shared_random,
};

use super::{
    under_water::Hl2DryUnderWater,
    launch::{self, BALL_MODEL, Hl2Rounds, SMG1_GRENADE_MODEL, U},
    recoil::{Hl2Punch, IMPULSE_SCALE},
};
use crate::{
    core::{DamageKind, FirstTimePredicted, Health, Intent, MovementState, NetRole, SimClock},
    map::PlaySound,
};

/// `hl2dm:smg1_grenade`'s parameters (Source units, seconds, points).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SmgGrenadeParams {
    #[serde(flatten)]
    pub rounds: launch::RoundsParams,
    pub speed: f32,
    pub gravity: f32,
    pub half: f32,
    pub damage: f32,
    pub radius: f32,
    pub primary: f32,
    pub secondary: f32,
    pub dry: f32,
    pub sound: String,
    pub empty: String,
    pub explode: String,
}

/// `hl2dm:combine_ball`'s parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BallParams {
    #[serde(flatten)]
    pub rounds: launch::RoundsParams,
    pub charge: f32,
    pub busy: f32,
    pub primary: f32,
    pub secondary: f32,
    pub dry: f32,
    pub speed: f32,
    pub radius: f32,
    pub mass: f32,
    pub life: f32,
    pub damage: f32,
    pub punch_pitch: [i32; 2],
    pub punch_yaw: [i32; 2],
    pub bounce_gap: f32,
    pub seek_cos: f32,
    pub seek_reach: f32,
    pub seek_half: f32,
    pub seek_kill_radius: f32,
    pub charge_sound: String,
    pub sound: String,
    pub launch_sound: String,
    pub empty: String,
    pub impact_sound: String,
    pub explode_sound: String,
}

/// The SMG1's grenade launcher.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2SmgGrenade(pub SmgGrenadeParams);

/// The AR2's ball: its charge's launch time.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Ball {
    pub params: BallParams,
    pub launch_at: Option<f64>,
}

impl NetPart for Hl2Ball {
    type Net = Option<f64>;
    fn net(&self) -> Option<f64> {
        self.launch_at
    }
    fn set_net(&mut self, at: Option<f64>) {
        self.launch_at = at;
    }
}

/// A ball in flight: when its last bounce counted, whether it has struck
/// a player.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BallFlight {
    pub last_bounce: Option<f64>,
    pub struck: bool,
}

/// Where projectiles are made here: the server, single player (never a
/// client's prediction), the first time a command runs.
pub fn makes_projectiles(role: Option<&NetRole>, first: &FirstTimePredicted) -> bool {
    role.is_none_or(|r| *r != NetRole::Client) && first.0
}

fn sound(play: &mut MessageWriter<PlaySound>, first: &FirstTimePredicted, entry: &str, at: Vec3, owner: Entity) {
    if first.0 {
        play.write(PlaySound {
            pitch: None,
            entry: entry.to_string(),
            at: Some(at),
            volume: None,
            source: Some(owner),
            channel: Some(chimera_sim::weapon::CHAN_WEAPON),
        });
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct Out<'w> {
    play: MessageWriter<'w, PlaySound>,
    events: MessageWriter<'w, WeaponEvent>,
    launch: MessageWriter<'w, LaunchRequest>,
    clock: Res<'w, SimClock>,
    first: Res<'w, FirstTimePredicted>,
    role: Option<Res<'w, NetRole>>,
}

impl Out<'_> {
    fn event(&mut self, owner: Entity, weapon: Entity, kind: WeaponEventKind) {
        let replay = !self.first.0;
        self.events.write(WeaponEvent {
            owner,
            weapon,
            kind,
            replay,
        });
    }

    fn makes(&self) -> bool {
        makes_projectiles(self.role.as_deref(), &self.first)
    }
}

/// Before the weapon frame: the SMG1's and the AR2's attack2.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<
        (
            Entity,
            &Intent,
            &mut Inventory,
            &Transform,
            &MovementState,
            Option<&ViewPunch>,
            Option<&mut Hl2Punch>,
            Has<LaunchUnderWater>,
            &Health,
        ),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut smgs: Query<(&Hl2SmgGrenade, &mut Hl2Rounds, &mut WeaponState, Option<&Hl2DryUnderWater>), Without<Hl2Ball>>,
    mut balls: Query<(&mut Hl2Ball, &mut Hl2Rounds, &mut WeaponState, Option<&Hl2DryUnderWater>), Without<Hl2SmgGrenade>>,
    mut out: Out,
    mut commands: Commands,
) {
    let now = out.clock.now;
    let due = now + 1e-5;
    for (owner, intent, mut inv, at, state, punch, mut velocity, wet, health) in &mut owners {
        if health.current <= 0.0 {
            continue;
        }
        let Some(active) = inv.active else { continue };
        if let Ok((g, mut rounds, mut st, dry)) = smgs.get_mut(active) {
            let p = &g.0;
            if !intent.secondary {
                continue;
            }
            let (eye, fwd, ..) = launch::aim(at, state, intent, punch, true);
            let free = due >= inv.next_attack;
            // A reload doesn't stop it (it cancels the reload).
            let ready = st.reloading() || (free && st.next_secondary <= due);
            if rounds.count == 0 {
                // No grenade: the generic "no secondary ammo" step, at most
                // every `dry`; the frame goes on.
                if ready {
                    sound(&mut out.play, &out.first, &p.empty, eye, owner);
                    st.next_secondary = now + p.dry as f64;
                }
                continue;
            }
            if wet {
                if free {
                    refuse_wet(&mut out, &mut inv, &mut st, dry, &p.empty, eye, owner);
                }
                continue;
            }
            if !ready {
                continue;
            }
            rounds.count -= 1;
            if st.reloading() {
                st.reload_end = None;
                inv.next_attack = now;
            }
            st.next_primary = now + p.primary as f64;
            st.next_secondary = now + p.secondary as f64;
            sound(&mut out.play, &out.first, &p.sound, eye, owner);
            out.event(owner, active, WeaponEventKind::Action("grenade"));
            if out.makes() {
                let mut f = Flyer::new(super::SMG1, Some(owner), fwd * p.speed * U, &out.clock);
                f.gravity = p.gravity * U;
                f.half = Vec3::splat(p.half * U);
                f.pass_items = true;
                f.touch = Touch::Explode;
                f.blast = Some(launch::blast(p.damage, p.radius, None, Some(&p.explode)));
                out.launch.write(LaunchRequest {
                    flyer: f,
                    start: eye,
                    shown: Some(SMG1_GRENADE_MODEL.into()),
                    tag: None,
                });
            }
            continue;
        }
        let Ok((mut ball, mut rounds, mut st, dry)) = balls.get_mut(active) else {
            continue;
        };
        let p = ball.params.clone();
        if let Some(t) = ball.launch_at {
            if now <= t + 1e-6 {
                continue;
            }
            // The launch (5.1).
            ball.launch_at = None;
            rounds.count = rounds.count.saturating_sub(1);
            inv.next_attack = now + p.busy as f64;
            st.next_primary = now + p.primary as f64;
            st.next_secondary = now + p.secondary as f64;
            // Up and to the left: pitch -8..-12, yaw 1..2 (whole degrees).
            let mut rng = shared_random(intent.command, "hl2dm:combine_ball", 0);
            let whole = |lo: i32, hi: i32, u: f32| lo + ((hi - lo + 1) as f32 * u).floor().min((hi - lo) as f32) as i32;
            let kick = Vec3::new(
                whole(p.punch_pitch[0], p.punch_pitch[1], rng.next()) as f32,
                whole(p.punch_yaw[0], p.punch_yaw[1], rng.next()) as f32,
                0.0,
            );
            let (eye, fwd, ..) = launch::aim(at, state, intent, punch, true);
            // A view punch: its velocity on HL2's spring (`recoil`).
            match velocity.as_deref_mut() {
                Some(v) => v.velocity += kick * IMPULSE_SCALE,
                None => {
                    commands.entity(owner).insert(Hl2Punch {
                        velocity: kick * IMPULSE_SCALE,
                    });
                    if punch.is_none() {
                        commands.entity(owner).insert(ViewPunch::default());
                    }
                }
            }
            sound(&mut out.play, &out.first, &p.sound, eye, owner);
            out.event(owner, active, WeaponEventKind::Action("ball"));
            if out.makes() {
                let mut f = Flyer::new(super::AR2, Some(owner), fwd * p.speed * U, &out.clock);
                f.half = Vec3::splat(p.radius * U);
                f.sphere = true;
                f.keep_speed = Some(p.speed * U);
                f.expires = Some(now + p.life as f64);
                f.touch = Touch::Bounce;
                // 5.2: its end at once on a sky surface.
                f.sky_end = Some(SkyEnd { probe: p.radius * 2.0 * U, expired: true });
                f.direct = Some(Direct {
                    damage: p.damage * launch::HP,
                    kind: DamageKind::Crush,
                    head: 1.0,
                    // The suit's rule: 800 to health on 200 armour.
                    armor_ratio: Some(1.0),
                    quantum: 0.0,
                    force: 0.0,
                });
                out.launch.write(LaunchRequest {
                    flyer: f,
                    start: eye,
                    shown: Some(BALL_MODEL.into()),
                    tag: None,
                });
                sound(&mut out.play, &out.first, &p.launch_sound, eye, owner);
            }
            continue;
        }
        if !intent.secondary || due < inv.next_attack {
            continue;
        }
        let (eye, ..) = launch::aim(at, state, intent, None, false);
        let ready = st.next_secondary <= due;
        if rounds.count == 0 {
            if ready {
                sound(&mut out.play, &out.first, &p.empty, eye, owner);
                st.next_secondary = now + p.dry as f64;
            }
            continue;
        }
        if wet {
            refuse_wet(&mut out, &mut inv, &mut st, dry, &p.empty, eye, owner);
            continue;
        }
        if !ready {
            continue;
        }
        // The charge (5.1).
        ball.launch_at = Some(now + p.charge as f64);
        st.next_primary = now + p.charge as f64;
        st.next_secondary = now + p.charge as f64;
        sound(&mut out.play, &out.first, &p.charge_sound, eye, owner);
        out.event(owner, active, WeaponEventKind::Action("charge"));
    }
}

/// The generic frame's refusal of attack2 under water (weapons.md 5,
/// "Secondary fire under water"): with no timer in front of it, every tick
/// attack2 is held: the empty sound, next secondary + the wait (0.2 s), and
/// the frame ends (no primary, no reload this tick). No dry-fire animation.
#[allow(clippy::too_many_arguments)]
fn refuse_wet(
    out: &mut Out,
    inv: &mut Inventory,
    st: &mut WeaponState,
    dry: Option<&Hl2DryUnderWater>,
    empty: &str,
    eye: Vec3,
    owner: Entity,
) {
    sound(&mut out.play, &out.first, empty, eye, owner);
    st.next_secondary = out.clock.now + dry.map_or(0.2, |d| d.wait) as f64;
    inv.frame_ended = true;
}

/// A charge in hand can't be put away; a charge no longer in hand (its
/// owner died) is dropped with no ball.
pub fn lock_charging(
    owners: Query<(&Inventory, &Health)>,
    mut balls: Query<(Entity, &mut Hl2Ball, &chimera_sim::weapon::Weapon, Has<HolsterLocked>)>,
    mut commands: Commands,
) {
    for (e, mut ball, w, locked) in &mut balls {
        let held = w
            .owner
            .and_then(|o| owners.get(o).ok())
            .is_some_and(|(inv, h)| inv.active == Some(e) && h.current > 0.0);
        if !held && ball.launch_at.is_some() {
            ball.launch_at = None;
        }
        let lock = ball.launch_at.is_some();
        if lock && !locked {
            commands.entity(e).insert(HolsterLocked);
        } else if !lock && locked {
            commands.entity(e).remove::<HolsterLocked>();
        }
    }
}

/// The ball's touches (the server's, `FlyerSet::Touch`): a world bounce
/// counts and seeks (5.4), a player struck is remembered; its sounds.
#[allow(clippy::type_complexity)]
pub fn ball_touches(
    mut hits: MessageReader<FlyerHit>,
    mut flyers: Query<(&mut Flyer, Option<&mut BallFlight>, &Transform)>,
    players: Query<(Entity, &Transform, &MovementState, &Health, Option<&crate::core::Team>), With<Intent>>,
    mut params: Local<Option<BallParams>>,
    game: Option<Res<crate::game::rules::Hl2Match>>,
    spatial: avian3d::prelude::SpatialQuery,
    colliders: Query<&avian3d::prelude::ColliderOf>,
    bodies: Query<(&avian3d::prelude::RigidBody, Option<&avian3d::prelude::ComputedMass>)>,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    for h in hits.read() {
        if h.weapon != super::AR2 || h.gone {
            continue;
        }
        // From the weapon's data: the ball can outlive every AR2.
        let p = params.get_or_insert_with(|| launch::hook_params(super::AR2, super::BALL_HOOK)).clone();
        // What moves is knocked (5.3): a whole elastic hit of its 150 kg
        // along the contact's normal.
        if let Some(other) = h.other.filter(|_| !h.character)
            && let Ok((rb, mass)) = bodies.get(other)
            && rb.is_dynamic()
        {
            let m = mass.map_or(1.0, |m| m.value()).max(1e-3);
            let into = h.velocity.dot(-h.normal).max(0.0);
            let dv = -h.normal * into * 2.0 * p.mass / (p.mass + m);
            commands.queue(move |w: &mut World| super::physcannon::push(w, other, dv, Vec3::ZERO));
        }
        let Ok((mut f, flight, t)) = flyers.get_mut(h.flyer) else {
            continue;
        };
        let mut state = flight.as_deref().copied().unwrap_or_default();
        if h.character {
            state.struck = true;
        } else if h.world {
            play.write(PlaySound::at(p.impact_sound.clone(), t.translation));
            let counts = state
                .last_bounce
                .is_none_or(|l| clock.now - l >= p.bounce_gap as f64 - 1e-6);
            if counts {
                state.last_bounce = Some(clock.now);
                let d = f.velocity.normalize_or_zero();
                let speed = f.velocity.length();
                // Attractive (5.4): alive, not the owner, not its teammate
                // in TDM, nobody in the intermission's freeze.
                let teamplay = game.as_ref().is_some_and(|m| m.teamplay);
                let frozen = game.as_ref().is_some_and(|m| m.over);
                let owner_team = f.owner.and_then(|o| players.get(o).ok()).and_then(|(.., t)| t.copied());
                let targets = players
                    .iter()
                    .filter(|(e, .., hp, _)| Some(*e) != f.owner && hp.current > 0.0 && !frozen)
                    .filter(|(.., team)| !(teamplay && team.is_some() && team.copied() == owner_team))
                    .map(|(e, at, s, ..)| Seekable {
                        entity: e,
                        min: at.translation + s.hull_min,
                        max: at.translation + s.hull_max,
                        eye: at.translation + s.eye_offset,
                    });
                if let Some(c) = seek(&p, h.point, d, state.struck, t.translation, targets, |from, to, e| {
                    clear_to(&spatial, &colliders, from, to, e)
                }) {
                    f.velocity = (c - h.point).normalize_or(d) * speed;
                }
            }
        }
        match flight {
            Some(mut fl) => *fl = state,
            None => {
                commands.entity(h.flyer).insert(state);
            }
        }
    }
}

/// The contact grenade's check (4.2), every 3 ticks of flight (the
/// server's, `FlyerSet::Touch`): one whose speed is exactly 0 explodes
/// where it is.
pub fn grenade_stopped(
    flyers: Query<(Entity, &Flyer, &Transform)>,
    mut blasts: MessageWriter<chimera_sim::weapon::grenade::BlastAt>,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for (e, f, t) in &flyers {
        if f.weapon != super::SMG1 || f.ticks == 0 || f.ticks % 3 != 0 || f.velocity != Vec3::ZERO {
            continue;
        }
        if let Some(b) = f.blast.clone() {
            blasts.write(chimera_sim::weapon::grenade::BlastAt {
                at: t.translation,
                blast: b,
                thrower: f.owner,
                weapon: f.weapon,
                inflictor: Some(e),
            });
        }
        gone.write(FlyerGone {
            flyer: e,
            weapon: f.weapon,
            owner: f.owner,
            at: t.translation,
            expired: false,
        });
        commands.entity(e).despawn();
    }
}

/// Whether a solid-only line from `from` to `to` is clear or ends on `e`.
fn clear_to(
    spatial: &avian3d::prelude::SpatialQuery,
    colliders: &Query<&avian3d::prelude::ColliderOf>,
    from: Vec3,
    to: Vec3,
    e: Entity,
) -> bool {
    use avian3d::prelude::SpatialQueryFilter;
    let d = to - from;
    let Ok(dir) = Dir3::new(d) else { return true };
    let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
    match spatial.cast_ray(from, dir, d.length(), true, &filter) {
        None => true,
        Some(h) => colliders.get(h.entity).map_or(h.entity, |c| c.body) == e,
    }
}

/// A player the ball may seek: its bounds and its eye (world space).
#[derive(Clone, Copy, Debug)]
pub struct Seekable {
    pub entity: Entity,
    pub min: Vec3,
    pub max: Vec3,
    pub eye: Vec3,
}

/// The seek after a counted bounce at `q` with the new direction `d`
/// (5.4): the winner's bounds centre among the attractive `targets`
/// (before it struck a player: those whose bounds meet a box of half
/// extent `seek_half` swept from `q` `seek_reach` along `d`, within
/// `seek_cos` of it; after: within `seek_kill_radius`, in front), `visible`
/// from the ball's centre `ball` to the eye; the nearest wins.
pub fn seek(
    p: &BallParams,
    q: Vec3,
    d: Vec3,
    struck: bool,
    ball: Vec3,
    targets: impl Iterator<Item = Seekable>,
    visible: impl Fn(Vec3, Vec3, Entity) -> bool,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for t in targets {
        let (e, eye) = (t.entity, t.eye);
        let c = (t.min + t.max) / 2.0;
        let to = c - q;
        let cos = to.normalize_or_zero().dot(d);
        let ok = if struck {
            to.length() <= p.seek_kill_radius * U && cos > 0.0
        } else {
            // The swept box meets the bounds: the segment meets the bounds
            // grown by the box's half extent.
            let grow = Vec3::splat(p.seek_half * U);
            let swept = launch_ray_box(t.min - grow, t.max + grow, q, d, p.seek_reach * U).is_some();
            swept && cos > p.seek_cos
        };
        if !ok || !visible(ball, eye, e) {
            continue;
        }
        let dist = to.length();
        if best.is_none_or(|(b, _)| dist < b) {
            best = Some((dist, c));
        }
    }
    best.map(|(_, c)| c)
}

fn launch_ray_box(min: Vec3, max: Vec3, from: Vec3, dir: Vec3, limit: f32) -> Option<f32> {
    chimera_sim::weapon::projectile::ray_box(min, max, from, dir, limit)
}

/// A ball's harmless end (5.6): its sound where it was.
pub fn ball_ends(mut gone: MessageReader<FlyerGone>, mut sound: Local<Option<String>>, mut play: MessageWriter<PlaySound>) {
    for g in gone.read() {
        if g.weapon == super::AR2 && g.expired {
            // From the weapon's data: the ball can outlive every AR2.
            let s = sound.get_or_insert_with(|| launch::hook_params::<BallParams>(super::AR2, super::BALL_HOOK).explode_sound);
            play.write(PlaySound::at(s.clone(), g.at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> BallParams {
        launch::hook_params(super::super::AR2, super::super::BALL_HOOK)
    }

    /// A standing player's bounds (±16, 72 tall) centred at `c` (units).
    fn at(c: Vec3) -> Seekable {
        Seekable {
            entity: Entity::PLACEHOLDER,
            min: (c - Vec3::new(16.0, 36.0, 16.0)) * U,
            max: (c + Vec3::new(16.0, 36.0, 16.0)) * U,
            eye: c * U,
        }
    }

    /// projectiles.md test 24: a player 15 degrees off is sought, one
    /// further off isn't.
    #[test]
    fn seeks_within_fifteen_degrees() {
        let p = params();
        let target = |y: f32| std::iter::once(at(Vec3::new(1000.0, 0.0, y)));
        let c = seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, target(200.0), |_, _, _| true).unwrap();
        let d = c.normalize();
        assert!((d - Vec3::new(0.98058, 0.0, 0.19612)).length() < 1e-4, "{d}");
        assert!(seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, target(300.0), |_, _, _| true).is_none());
    }

    /// 5.4: the sweep is a box of half extent 256 from Q to Q + 2048 d, so
    /// bounds just past its end still meet it (a cylinder along the line
    /// stopped at 2048); past the box's reach nothing.
    #[test]
    fn the_seek_sweeps_a_box() {
        let p = params();
        let one = |x: f32| std::iter::once(at(Vec3::new(x, 0.0, 0.0)));
        assert!(seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, one(2250.0), |_, _, _| true).is_some());
        assert!(seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, one(2330.0), |_, _, _| true).is_none());
    }

    /// Test 26: after a kill, the nearest in front within 512.
    #[test]
    fn after_a_kill_seeks_in_front() {
        let p = params();
        let front = at(Vec3::new(400.0, 0.0, 0.0));
        let behind = at(Vec3::new(-300.0, 0.0, 0.0));
        let c = seek(&p, Vec3::ZERO, Vec3::X, true, Vec3::ZERO, [front, behind].into_iter(), |_, _, _| true).unwrap();
        assert!((c - (front.min + front.max) / 2.0).length() < 1e-5);
    }
}
