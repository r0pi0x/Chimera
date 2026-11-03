//! The machine guns' attack2s (specs/hl2dm/projectiles.md 4, 5): the
//! SMG1's contact grenade (`hl2dm:smg1_grenade`, `Hl2SmgGrenade`) and the
//! AR2's energy ball (`hl2dm:combine_ball`, `Hl2Ball`), each with its own
//! rounds (`launch::Hl2Rounds`: SMG1_Grenade 3, AR2AltFire 3; none at
//! spawn), predicted with the weapon; the projectiles are the server's
//! (Chimera's flyers, `weapon::projectile`).
//!
//! - **Contact grenade**: attack2 with the next secondary passed: with no
//!   grenade or the eyes under water a dry fire (`Weapon_SMG1.Empty`, next
//!   secondary + 0.5); else 1 used, a running reload cancelled (no ammo
//!   moved), next primary + 0.5, next secondary + 1.0, the player not made
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
//! Known shortcuts: the contact grenade's 3-tick "stopped or on the
//! ground" check isn't needed by a toss that always ends on a touch and
//! isn't done, nor its random spin; the ball's seek sweep is a cylinder
//! of radius 256 + 16 around its line (the spec's swept box), its
//! attractiveness has no team test (TDM is M7's), the ball's flash and the
//! shooter's white fade, the dissolve of its victims and their weapons
//! (M10) aren't drawn; the ball knocks props as a bounce (no 1000 crush
//! to props), and its harmless end is removed at once.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    HolsterLocked, Inventory, NetPart, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState,
    projectile::{Direct, Flyer, FlyerGone, FlyerHit, LaunchRequest, LaunchUnderWater, Touch},
    random::shared_random,
};

use super::{
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
    mut smgs: Query<(&Hl2SmgGrenade, &mut Hl2Rounds, &mut WeaponState), Without<Hl2Ball>>,
    mut balls: Query<(&mut Hl2Ball, &mut Hl2Rounds, &mut WeaponState), Without<Hl2SmgGrenade>>,
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
        if let Ok((g, mut rounds, mut st)) = smgs.get_mut(active) {
            let p = &g.0;
            // A reload doesn't stop it (it cancels the reload).
            let ready = st.reloading() || (due >= inv.next_attack && st.next_secondary <= due);
            if !intent.secondary || !ready {
                continue;
            }
            let (eye, fwd, ..) = launch::aim(at, state, intent, punch, true);
            if rounds.count == 0 || wet {
                sound(&mut out.play, &out.first, &p.empty, eye, owner);
                st.next_secondary = now + p.dry as f64;
                out.event(owner, active, WeaponEventKind::DryFire);
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
        let Ok((mut ball, mut rounds, mut st)) = balls.get_mut(active) else {
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
        if !intent.secondary || due < inv.next_attack || st.next_secondary > due {
            continue;
        }
        let (eye, ..) = launch::aim(at, state, intent, None, false);
        if rounds.count == 0 || wet {
            sound(&mut out.play, &out.first, &p.empty, eye, owner);
            st.next_secondary = now + p.dry as f64;
            out.event(owner, active, WeaponEventKind::DryFire);
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
    players: Query<(Entity, &Transform, &MovementState, &Health), With<Intent>>,
    weapons: Query<&Hl2Ball>,
    spatial: avian3d::prelude::SpatialQuery,
    colliders: Query<&avian3d::prelude::ColliderOf>,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut commands: Commands,
) {
    let Some(p) = weapons.iter().next().map(|b| b.params.clone()) else {
        hits.clear();
        return;
    };
    for h in hits.read() {
        if h.weapon != super::AR2 || h.gone {
            continue;
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
                let targets = players
                    .iter()
                    .filter(|(e, .., hp)| Some(*e) != f.owner && hp.current > 0.0)
                    .map(|(e, at, s, _)| {
                        (
                            e,
                            at.translation + (s.hull_min + s.hull_max) / 2.0,
                            at.translation + s.eye_offset,
                        )
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

/// The seek after a counted bounce at `q` with the new direction `d`
/// (5.4): the winner's centre among `targets` (entity, bounds centre,
/// eye), `visible` from the ball's centre `ball` to the eye.
pub fn seek(
    p: &BallParams,
    q: Vec3,
    d: Vec3,
    struck: bool,
    ball: Vec3,
    targets: impl Iterator<Item = (Entity, Vec3, Vec3)>,
    visible: impl Fn(Vec3, Vec3, Entity) -> bool,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for (e, c, eye) in targets {
        let to = c - q;
        let cos = to.normalize_or_zero().dot(d);
        let ok = if struck {
            to.length() <= p.seek_kill_radius * U && cos > 0.0
        } else {
            let along = to.dot(d);
            let off = (to - d * along).length();
            along >= 0.0 && along <= p.seek_reach * U && off <= (p.seek_half + 16.0) * U && cos > p.seek_cos
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

/// A ball's harmless end (5.6): its sound where it was.
pub fn ball_ends(mut gone: MessageReader<FlyerGone>, weapons: Query<&Hl2Ball>, mut play: MessageWriter<PlaySound>) {
    let sound = weapons.iter().next().map(|b| b.params.explode_sound.clone());
    for g in gone.read() {
        if g.weapon == super::AR2
            && g.expired
            && let Some(s) = &sound
        {
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

    /// projectiles.md test 24: a player 15 degrees off is sought, one
    /// further off isn't.
    #[test]
    fn seeks_within_fifteen_degrees() {
        let p = params();
        let u = U;
        let target = |y: f32| {
            std::iter::once((
                Entity::PLACEHOLDER,
                Vec3::new(1000.0, 0.0, y) * u,
                Vec3::new(1000.0, 0.0, y) * u,
            ))
        };
        let c = seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, target(200.0), |_, _, _| {
            true
        })
        .unwrap();
        let d = c.normalize();
        assert!((d - Vec3::new(0.98058, 0.0, 0.19612)).length() < 1e-4, "{d}");
        assert!(
            seek(&p, Vec3::ZERO, Vec3::X, false, Vec3::ZERO, target(300.0), |_, _, _| {
                true
            })
            .is_none()
        );
    }

    /// Test 26: after a kill, the nearest in front within 512.
    #[test]
    fn after_a_kill_seeks_in_front() {
        let p = params();
        let front = (Entity::PLACEHOLDER, Vec3::new(400.0, 0.0, 0.0) * U, Vec3::ZERO);
        let behind = (Entity::PLACEHOLDER, Vec3::new(-300.0, 0.0, 0.0) * U, Vec3::ZERO);
        let c = seek(
            &p,
            Vec3::ZERO,
            Vec3::X,
            true,
            Vec3::ZERO,
            [front, behind].into_iter(),
            |_, _, _| true,
        )
        .unwrap();
        assert!((c - front.1).length() < 1e-5);
    }
}
