//! The movement kit (specs/tf2/batch3b_mechanics.md 8, 9): the B.A.S.E.
//! Jumper's chute (`tf2:chute`, `ChuteParams`, the part `Tf2Chute`), the
//! stomp (the Mantreads' and the Thermal Thruster's), the Mantreads' push
//! resistances and the Thermal Thruster (`tf2:thruster`,
//! `ThrusterParams`, the part `Tf2Thruster`). The chute and the thruster
//! move their holder, so they are predicted (his client runs them, the
//! server's state a correction), in TF2's movement tick around Source's
//! step (`movement::select_class` first).
//!
//! - **The chute** (`chute`, before the step; a worn item, carried, never
//!   drawn: data/weapons/parachute.toml): jump pressed in the air opens it
//!   ("deployed" for the rest of the air time: it opens once); pressed
//!   again it closes and falls normally, and stays closed until he lands.
//!   Open, the vertical speed is held at `fall` (100) or less down before
//!   gravity's half-step (so he falls 106 units/s and reads 112 at a
//!   tick's end) and the horizontal speed goes `approach` (0.45) of the
//!   way to `air_speed` (75) along the keys' wish each tick (33.75 after
//!   one tick), to 0 with no key. Landing closes it.
//! - **The stomp** (`stomps`, the server's, after the step): a holder of a
//!   stomping item (`ItemParams::stomp`) landing on an enemy's head after
//!   falling faster than `STOMP_FROM` (650) deals 10 + 3 × H × v / 6000
//!   (H his own max health, v the fall speed in the landing's move: the
//!   last tick's end speed less gravity's half-step), rounded, its own
//!   kind (`DamageKind::Generic`), credited to the item; he takes no fall
//!   damage for that landing (`no_stomp_fall`).
//! - **The Mantreads' resistances** (`ItemParams::push_taken`,
//!   `airblast_taken`): another's push × 0.25 (`weapons::blast`), the
//!   airblast's power × 0.25 before its ground lift (`weapons::airblast`,
//!   `airblast_taken`).
//! - **The Thermal Thruster** (`thruster`, before the step; `landed`,
//!   after it): attack with it out and a charge (meter ≥ `charge`, 50 of
//!   100) hops (the vertical speed set to `hop`, 350) and `delay` (0.65 s)
//!   later launches: the velocity `speed` (810) along the aim plus
//!   `up_level` (315) up when the aim is within `level_band` degrees of
//!   level (a downward aim then counted as level), else plus `up_steep`
//!   (112.5); no attack for `stun` (0.5 s). In flight the horizontal speed
//!   holds (Source's air rules add nothing over it) and a press does
//!   nothing. The meter refills over `refill` s (30: 15 s a launch). On
//!   the landing tick the horizontal speed is cut to `land_speed` (300);
//!   enemies within `push_radius` (220) of the landing are pushed away
//!   `push` (280 out, 278 up, `landing_push`, the server's) and take no
//!   damage; a stomp is the stomp's. Its meter shows as the item meter.
//!
//! Known shortcuts (the spec's open questions, provisional): the chute's
//! approach rate is fitted on the one 33.7 sample, an entry speed above
//! 75 is pulled down by the same rate (400 measured 298 at once); the
//! thruster's upward parts' boundary is `level_band` (15°, between the
//! measured 10° and 20°, the same down), its landing push direction law is
//! straight out from the landing at the measured mid values, its stun only
//! holds attacks, its `damage_force_reduction` (1.5) and the taunt
//! attack aren't applied, re-launching in flight (an attribute) isn't
//! built; the Mantreads' blast-jump air control (× 3, unmeasured) is
//! recorded, not applied; the stomp needs the bodies' blocking (enemies
//! block each other) and its look and sound aren't played.

use bevy::{platform::collections::HashMap, prelude::*};
use serde::{Deserialize, Serialize};

use avian3d::prelude::ColliderAabb;
use chimera_sim::weapon::{Inventory, NetPart, Weapon};

use super::mods::Tf2Item;
use crate::core::{Damage, DamageKind, ExternalPush, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, SimClock, Team, Velocity};
use crate::game::weapons::{blast::boxed, damage::U};

/// The hooks.
pub const CHUTE_HOOK: &str = "tf2:chute";
pub const THRUSTER_HOOK: &str = "tf2:thruster";
/// A stomp needs a fall faster than this (units/s).
pub const STOMP_FROM: f32 = 650.0;
/// The stomp's law (§9.1): base and the fall damage's factor.
pub const STOMP_BASE: f32 = 10.0;
pub const STOMP_MULT: f32 = 3.0;
/// TF2's gravity (units/s²: the half-step taken off the end speed).
pub const GRAVITY: f32 = 800.0;

/// The airblast power `inv`'s items let through (the Mantreads' 0.25).
pub fn airblast_taken(inv: Option<&Inventory>, items: &Query<&Tf2Item>) -> f32 {
    inv.map_or(1.0, |inv| inv.weapons.iter().filter_map(|w| items.get(*w).ok()).map(|i| i.0.airblast_taken).product())
}

/// The stomp's damage for a fall at `v` (units/s, the landing move's) by
/// a stomper of max health `max` (points).
pub fn stomp_damage(max: f32, v: f32) -> f32 {
    (STOMP_BASE + STOMP_MULT * max * v / 6000.0).round()
}

/// The thruster's launch velocity (units/s, engine axes: y up) for a view
/// pitch `pitch` (radians, up positive) and yaw.
pub fn launch_velocity(p: &ThrusterParams, yaw: f32, pitch: f32) -> Vec3 {
    let band = p.level_band.to_radians();
    let level = pitch.abs() <= band;
    let pitch = if level { pitch.max(0.0) } else { pitch };
    let up = if level { p.up_level } else { p.up_steep };
    let flat = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    flat * p.speed * pitch.cos() + Vec3::Y * (p.speed * pitch.sin() + up)
}

// ---- The chute ----

/// `tf2:chute`'s parameters (units/s; module doc).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ChuteParams {
    pub fall: f32,
    pub air_speed: f32,
    pub approach: f32,
}

impl Default for ChuteParams {
    fn default() -> Self {
        Self { fall: 100.0, air_speed: 75.0, approach: 0.45 }
    }
}

/// The chute's state (predicted).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChuteState {
    pub open: bool,
    pub deployed: bool,
    pub prev_jump: bool,
}

/// The chute's part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Chute {
    pub params: ChuteParams,
    pub state: ChuteState,
}

impl NetPart for Tf2Chute {
    type Net = ChuteState;
    fn net(&self) -> ChuteState {
        self.state
    }
    fn set_net(&mut self, net: ChuteState) {
        self.state = net;
    }
}

/// The keys' wish (unit, flat; zero with none).
fn wish(intent: &Intent) -> Vec3 {
    let r = intent.yaw_rotation();
    let (fwd, right) = (r * Vec3::NEG_Z, r * Vec3::X);
    (fwd * intent.move_axis.y + right * intent.move_axis.x).with_y(0.0).normalize_or_zero()
}

/// Each chute (module doc), before the step.
#[allow(clippy::type_complexity)]
pub fn chute(
    mut chutes: Query<(&mut Tf2Chute, &Weapon)>,
    mut owners: Query<(&Intent, &mut Velocity, Option<&MovementState>, Option<&Health>), Without<crate::map::interp::NetDrawn>>,
) {
    for (mut c, weapon) in &mut chutes {
        let Some(owner) = weapon.owner else { continue };
        let Ok((intent, mut velocity, state, health)) = owners.get_mut(owner) else { continue };
        let p = c.params.clone();
        let mut s = c.state;
        let grounded = state.is_none_or(|m| m.on_ground) || health.is_some_and(|h| h.current <= 0.0);
        let press = intent.jump && !s.prev_jump;
        s.prev_jump = intent.jump;
        if grounded {
            s.open = false;
            s.deployed = false;
        } else if press {
            if s.open {
                s.open = false;
            } else if !s.deployed {
                s.open = true;
                s.deployed = true;
            }
        }
        if s.open {
            let mut v = velocity.0 / U;
            v.y = v.y.max(-p.fall);
            let flat = Vec3::new(v.x, 0.0, v.z);
            let w = wish(intent);
            let flat = if w == Vec3::ZERO { Vec3::ZERO } else { flat + (w * p.air_speed - flat) * p.approach };
            let v = Vec3::new(flat.x, v.y, flat.z) * U;
            if velocity.0 != v {
                velocity.0 = v;
            }
        }
        if c.state != s {
            c.state = s;
        }
    }
}

// ---- The stomp ----

/// Each stomper's fall speed at the last tick's end (units/s, down
/// positive) and the stomps of this tick (the server's).
#[derive(Clone, Debug, Default)]
pub struct Falls {
    pub last: HashMap<Entity, f32>,
    pub stomped: Vec<Entity>,
}

/// The stomp (module doc; the server's, after the step).
#[allow(clippy::type_complexity)]
pub fn stomps(
    holders: Query<(Entity, &Inventory, &Transform, Option<&ColliderAabb>, &MovementState, &Velocity, &Health, Option<&Team>)>,
    chars: Query<(Entity, &Transform, Option<&ColliderAabb>, Option<&MovementState>, &Health, Option<&Team>), With<Intent>>,
    items: Query<(&Tf2Item, &Weapon)>,
    friendly: Option<Res<crate::core::FriendlyFire>>,
    mut state: ResMut<super::ItemState>,
    clock: Res<SimClock>,
    mut out: MessageWriter<Damage>,
) {
    let falls = &mut state.falls;
    let ff = friendly.is_some_and(|f| f.0 != 0);
    let dt = clock.dt();
    falls.stomped.clear();
    let mut seen = Vec::new();
    for (e, inv, at, aabb, state, velocity, health, team) in &holders {
        let Some(item) = inv.weapons.iter().find_map(|w| items.get(*w).ok().filter(|(i, _)| i.0.stomp)).map(|(_, w)| w.id) else {
            continue;
        };
        seen.push(e);
        let last = falls.last.get(&e).copied().unwrap_or(0.0);
        falls.last.insert(e, (-velocity.0.y / U).max(0.0));
        let v = last - GRAVITY * dt / 2.0;
        if !state.on_ground || v <= STOMP_FROM || health.current <= 0.0 {
            continue;
        }
        let (min, max) = boxed(at, aabb, Some(state));
        let team = team.copied().filter(|t| t.0 != 0);
        let under = chars.iter().find(|(o, t, b, s, h, their)| {
            if *o == e || h.current <= 0.0 || (!ff && team.is_some() && their.copied() == team) {
                return false;
            }
            let (lo, hi) = boxed(t, *b, *s);
            (min.y - hi.y).abs() <= 4.0 * U && min.x < hi.x && max.x > lo.x && min.z < hi.z && max.z > lo.z
        });
        let Some((victim, ..)) = under else { continue };
        out.write(Damage {
            target: victim,
            attacker: Some(e),
            amount: stomp_damage(health.max * 100.0, v) / 100.0,
            point: (min + max) / 2.0,
            dir: Vec3::NEG_Y,
            hitgroup: Hitgroup::Head,
            kind: DamageKind::Generic,
            weapon: Some(item),
            force: Vec3::ZERO,
        });
        falls.stomped.push(e);
    }
    falls.last.retain(|e, _| seen.contains(e));
}

/// A stomper takes no fall damage for the landing that stomped.
pub fn no_stomp_fall(mut damage: MessageMutator<Damage>, state: Res<super::ItemState>) {
    let falls = &state.falls;
    if falls.stomped.is_empty() {
        return;
    }
    for d in damage.read() {
        if d.kind == DamageKind::Fall && falls.stomped.contains(&d.target) {
            d.amount = 0.0;
        }
    }
}

// ---- The Thermal Thruster ----

/// `tf2:thruster`'s parameters (units/s, s, meter points, degrees).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ThrusterParams {
    pub hop: f32,
    pub delay: f32,
    pub speed: f32,
    pub up_level: f32,
    pub up_steep: f32,
    pub level_band: f32,
    pub charge: f32,
    pub refill: f32,
    pub stun: f32,
    pub land_speed: f32,
    pub push_radius: f32,
    pub push: [f32; 2],
}

impl Default for ThrusterParams {
    fn default() -> Self {
        Self {
            hop: 350.0,
            delay: 0.65,
            speed: 810.0,
            up_level: 315.0,
            up_steep: 112.5,
            level_band: 15.0,
            charge: 50.0,
            refill: 30.0,
            stun: 0.5,
            land_speed: 300.0,
            push_radius: 220.0,
            push: [280.0, 278.0],
        }
    }
}

/// The thruster's state (predicted).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThrusterState {
    /// 0 to 100.
    pub meter: f32,
    /// The launch due (sim time; none: negative).
    pub launch_at: f64,
    pub flying: bool,
    pub prev_fire: bool,
}

impl Default for ThrusterState {
    fn default() -> Self {
        Self { meter: 100.0, launch_at: -1.0, flying: false, prev_fire: false }
    }
}

/// The thruster's part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Thruster {
    pub params: ThrusterParams,
    pub state: ThrusterState,
}

impl NetPart for Tf2Thruster {
    type Net = ThrusterState;
    fn net(&self) -> ThrusterState {
        self.state
    }
    fn set_net(&mut self, net: ThrusterState) {
        self.state = net;
    }
}

impl Tf2Thruster {
    /// The HUD meter: its label and fill.
    pub fn meter(&self) -> (&str, f32) {
        ("THRUSTER", (self.state.meter / 100.0).clamp(0.0, 1.0))
    }
}

/// A thruster's landing (for the server's push).
#[derive(Clone, Copy, Debug)]
pub struct ThrusterLanded {
    pub pyro: Entity,
    pub weapon: Entity,
    pub at: Vec3,
}

/// Each thruster's meter, hop and launch (module doc), before the step.
#[allow(clippy::type_complexity)]
pub fn thruster(
    mut packs: Query<(Entity, &mut Tf2Thruster, &Weapon)>,
    mut owners: Query<
        (&mut Intent, &mut Inventory, &mut Velocity, Option<&mut MovementState>, Option<&mut ExternalPush>, Option<&Health>),
        Without<crate::map::interp::NetDrawn>,
    >,
    clock: Res<SimClock>,
) {
    let (now, dt) = (clock.now, clock.dt());
    for (w, mut pack, weapon) in &mut packs {
        let Some(owner) = weapon.owner else { continue };
        let Ok((mut intent, mut inv, mut velocity, mut state, push, health)) = owners.get_mut(owner) else { continue };
        let p = pack.params.clone();
        let mut s = pack.state;
        if health.is_some_and(|h| h.current <= 0.0) {
            s = ThrusterState { prev_fire: s.prev_fire, ..default() };
        } else {
            s.meter = (s.meter + 100.0 / p.refill.max(1e-3) * dt).min(100.0);
            let active = inv.active == Some(w);
            let press = intent.fire && !s.prev_fire && active && inv.ready(now);
            let mut lift = false;
            if press && !s.flying && s.launch_at < 0.0 && s.meter >= p.charge - 1e-3 {
                s.meter -= p.charge;
                s.launch_at = now + p.delay as f64;
                velocity.0.y = p.hop * U;
                lift = true;
            }
            if s.launch_at >= 0.0 && now + 1e-6 >= s.launch_at {
                s.launch_at = -1.0;
                s.flying = true;
                velocity.0 = launch_velocity(&p, intent.yaw, intent.pitch) * U;
                inv.next_attack = inv.next_attack.max(now + p.stun as f64);
                lift = true;
            }
            if lift {
                if let Some(m) = state.as_mut().filter(|m| m.on_ground) {
                    m.on_ground = false;
                }
                if let Some(mut x) = push {
                    x.lift = true;
                }
            }
            if active {
                // Its own attack does nothing else.
                s.prev_fire = intent.fire;
                intent.fire = false;
            } else {
                s.prev_fire = intent.fire;
            }
        }
        if pack.state != s {
            pack.state = s;
        }
    }
}

/// Each thruster's landing: the horizontal speed cut, the server's push
/// told (after the step).
#[allow(clippy::type_complexity)]
pub fn landed(
    mut packs: Query<(Entity, &mut Tf2Thruster, &Weapon)>,
    mut owners: Query<(&Transform, &mut Velocity, Option<&MovementState>), Without<crate::map::interp::NetDrawn>>,
    first: Res<FirstTimePredicted>,
    role: Option<Res<crate::core::NetRole>>,
    mut items: ResMut<super::ItemState>,
) {
    for (w, mut pack, weapon) in &mut packs {
        if !pack.state.flying {
            continue;
        }
        let Some(owner) = weapon.owner else { continue };
        let Ok((at, mut velocity, state)) = owners.get_mut(owner) else { continue };
        if !state.is_some_and(|m| m.on_ground) {
            continue;
        }
        pack.state.flying = false;
        let flat = Vec3::new(velocity.0.x, 0.0, velocity.0.z);
        let most = pack.params.land_speed * U;
        if flat.length() > most {
            let cut = flat.normalize() * most;
            velocity.0.x = cut.x;
            velocity.0.z = cut.z;
        }
        if first.0 && super::consumables::serving(role.as_deref()) {
            items.landings.push(ThrusterLanded { pyro: owner, weapon: w, at: at.translation });
        }
    }
}

/// Enemies near a thruster's landing pushed away and up (the server's).
#[allow(clippy::type_complexity)]
pub fn landing_push(
    mut state: ResMut<super::ItemState>,
    packs: Query<&Tf2Thruster>,
    mut chars: Query<(Entity, &Transform, &Health, Option<&Team>, &mut Velocity, Option<&mut MovementState>, Option<&mut ExternalPush>), With<Intent>>,
    friendly: Option<Res<crate::core::FriendlyFire>>,
) {
    let ff = friendly.is_some_and(|f| f.0 != 0);
    let landings = std::mem::take(&mut state.landings);
    for l in landings {
        let Ok(pack) = packs.get(l.weapon) else { continue };
        let p = &pack.params;
        let team = chars.get(l.pyro).ok().and_then(|c| c.3.copied()).filter(|t| t.0 != 0);
        for (e, t, h, their, mut velocity, state, push) in &mut chars {
            if e == l.pyro || h.current <= 0.0 || (!ff && team.is_some() && their.copied() == team) {
                continue;
            }
            let to = (t.translation - l.at) / U;
            if to.length() > p.push_radius {
                continue;
            }
            let out = to.with_y(0.0).normalize_or(Vec3::NEG_Z);
            velocity.0 += (out * p.push[0] + Vec3::Y * p.push[1]) * U;
            if let Some(mut m) = state {
                m.on_ground = false;
            }
            if let Some(mut x) = push {
                x.lift = true;
            }
        }
    }
}

/// The hooks, the parts and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, data::WeaponDataAppExt};
    use source_engine::source_movement::MovementStep;
    app.register_weapon_hook::<ChuteParams>(CHUTE_HOOK, |e, _, p| {
            e.insert(Tf2Chute { params: p.clone(), state: ChuteState::default() });
        })
        .register_weapon_hook::<ThrusterParams>(THRUSTER_HOOK, |e, _, p| {
            e.insert(Tf2Thruster { params: p.clone(), state: ThrusterState::default() });
        })
        .predicted_part::<Tf2Chute>()
        .predicted_part::<Tf2Thruster>()
        .add_systems(
            crate::core::Predict::Movement,
            (
                (chute, thruster).after(crate::game::movement::select_class).before(MovementStep),
                landed.after(MovementStep),
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                (stomps, no_stomp_fall).chain().after(crate::core::SimSet::Movement).before(crate::core::apply_damage),
                landing_push.after(crate::core::SimSet::Movement).before(crate::core::SimSet::Weapons),
            )
                .run_if(crate::core::authoritative),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §9.1: Soldier (200) at 900 → 99 (the landing move's 894); Pyro
    /// (175) at 890 → 87; max health 100 at 1068 → 63.
    #[test]
    fn the_stomp_follows_9_1() {
        let v = |end: f32| end - GRAVITY * 0.015 / 2.0;
        assert_eq!(stomp_damage(200.0, v(900.0)), 99.0);
        assert_eq!(stomp_damage(200.0, v(708.0)), 80.0);
        assert_eq!(stomp_damage(200.0, v(1332.0)), 143.0);
        assert_eq!(stomp_damage(175.0, v(890.0)), 87.0);
        assert_eq!(stomp_damage(100.0, v(1068.0)), 63.0);
    }

    /// §9.3's table.
    #[test]
    fn the_launch_follows_the_table() {
        let p = ThrusterParams::default();
        let at = |deg: f32| {
            let v = launch_velocity(&p, 0.0, deg.to_radians());
            (Vec2::new(v.x, v.z).length(), v.y)
        };
        let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.2 && (a.1 - b.1).abs() < 0.2;
        assert!(near(at(0.0), (810.0, 315.0)));
        assert!(near(at(-5.0), (810.0, 315.0)), "{:?}", at(-5.0));
        assert!(near(at(5.0), (806.9, 385.6)), "{:?}", at(5.0));
        assert!(near(at(10.0), (797.7, 455.6)), "{:?}", at(10.0));
        assert!(near(at(20.0), (761.2, 389.5)), "{:?}", at(20.0));
        assert!(near(at(30.0), (701.5, 517.5)), "{:?}", at(30.0));
        assert!(near(at(60.0), (405.0, 814.0)), "{:?}", at(60.0));
        assert!(near(at(-20.0), (761.2, -164.5)), "{:?}", at(-20.0));
    }
}
