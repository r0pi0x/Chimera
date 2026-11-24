//! TF2's blood on a player hit (specs/tf2/effects.md 8), from
//! games/tf2/data/blood.toml (`BloodLooks`); Source's shared blood is off
//! for TF2's guns (`impact_effects::BloodView::Game`).
//!
//! - **The burst** (8a; the server's, `bleed`): one per damage event, the
//!   pellets of a blast on one victim one event (`Tf2Blood`, one per
//!   attacker and victim a tick), when a TF2 gun's damage lands (not a
//!   teammate's, not fire: a Burn, or an attacker holding the Flame
//!   Thrower; not damage of no kind). Where: a bullet's hit point; a melee
//!   hit at the attacker's eye + 48 along his view; a blast at its centre
//!   (the frame's `Tf2Explosion` nearest the victim). Its axis toward the
//!   inflictor's centre 10 units lowered (the attacker's, the blast's).
//!   Every machine that draws shows it (`ToClients`), held for the
//!   `LateDelay` (8a "Timing", 11): `water_blood_impact_red_01` alone with
//!   the victim's eyes under water, else `blood_impact_red_01` and a spray
//!   each viewer picks (`spray_axis`, `spray_name`: `_far` from 400 units,
//!   pushed sideways when along his view); `lowV_` ones with
//!   `violence_hblood 0`.
//! - **The stains** (8c; `Tf2Stains`): each bullet or pellet trace that
//!   hit an enemy (`WeaponEvent` shots), and each melee hit, traces behind
//!   the victim by the gun's own per-hit damage (before range and crit:
//!   `DamageEffect::amount`; a swing's damage), 172 units
//!   onward, `Blood` decals (`impact_effects::BloodPainter::stains`),
//!   none with `violence_hblood 0`.
//!
//! - **The body decals** (8b): each TF2 bullet, pellet or swing into an
//!   enemy decals the body (`ImpactStyle::body_decals` in `looks::STYLE`,
//!   drawn by Chimera's `render::body_decals`), none in low violence; here
//!   their TF2 rules: healed to full removes them (`body_decal_rules`), the
//!   ragdoll doesn't keep them (`movement::ClassParts`).
//!
//! Known shortcuts: no Über yet; the Spy's cloak and disguise
//! (`items::spy`) don't yet insert `map::NoBodyDecals` nor write
//! `ClearBodyDecals` on a disguise change; needles' body decal (12) not made; no birthday or Pyro vision; low
//! violence is `violence_hblood` alone (8: four cvars); the stains aren't
//! held for the delay; the victim's and attacker's centres are their
//! origins.

use chimera_core::game_data::GameData;
use bevy::platform::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_core::core::{Damage, DamageKind, FriendlyFire, Health, Intent, MapWater, MovementState, Team, refused_by_team};
use chimera_sim::weapon::{DamageEffect, Inventory, Weapon, WeaponEvent, WeaponEventKind, aim_of};
use source_engine::effects::impact_effects::{BloodPainter, ImpactEffectSettings, bleed_traces};

use super::{blast::Tf2Explosion, damage::U, looks::in_water};
use super::super::classes::Tf2Class;
use crate::map::{ClearBodyDecals, SkyboxCamera, loose::LateDelay, view_model::ViewModelCamera};

/// The data.
pub const DATA: GameData = chimera_core::game_data!("data/blood.toml");

/// TF2's blood systems and numbers (data/blood.toml).
#[derive(Resource, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BloodLooks {
    pub impact: String,
    pub impact_low: String,
    pub water: String,
    pub water_low: String,
    pub spray: String,
    pub spray_far: String,
    pub spray_low: String,
    pub spray_far_low: String,
    pub far: f32,
    pub push_above: f32,
    pub push_per: f32,
    pub melee_reach: f32,
    pub inflictor_lower: f32,
}

impl Default for BloodLooks {
    fn default() -> Self {
        toml::from_str(&DATA).expect("data/blood.toml")
    }
}

impl BloodLooks {
    /// Every system it can start (for the loaders).
    pub fn systems(&self) -> Vec<String> {
        [&self.impact, &self.impact_low, &self.water, &self.water_low, &self.spray, &self.spray_far, &self.spray_low, &self.spray_far_low]
            .into_iter()
            .cloned()
            .collect()
    }

    /// The burst's system: under water's or not, low violence's or not.
    pub fn burst(&self, under_water: bool, low: bool) -> &str {
        match (under_water, low) {
            (true, false) => &self.water,
            (true, true) => &self.water_low,
            (false, false) => &self.impact,
            (false, true) => &self.impact_low,
        }
    }

    /// The spray's system for a viewer `distance` units away.
    pub fn spray_name(&self, distance: f32, low: bool) -> &str {
        match (distance >= self.far, low) {
            (false, false) => &self.spray,
            (true, false) => &self.spray_far,
            (false, true) => &self.spray_low,
            (true, true) => &self.spray_far_low,
        }
    }

    /// The spray's axis (8a) for a burst toward `toward` (unit) seen by a
    /// viewer looking along `forward` with `right` from `distance` units:
    /// out the far side (−toward), pushed sideways by right x k (k = `k0`
    /// in [0.5, 1.5] + 0.25 x distance / 512) when along the view: to the
    /// right from 512 units when it leans right, below 512 when `coin`,
    /// else left. Not renormalised (only its direction counts).
    pub fn spray_axis(&self, toward: Vec3, forward: Vec3, right: Vec3, distance: f32, k0: f32, coin: bool) -> Vec3 {
        let s = -toward;
        if s.dot(forward).abs() <= self.push_above {
            return s;
        }
        let k = k0 + 0.25 * distance / self.push_per;
        let to_right = if distance >= self.push_per { s.dot(right) > 0.0 } else { coin };
        if to_right { s + right * k } else { s - right * k }
    }
}

/// A burst to draw (8a): where, its axis toward the inflictor (unit), and
/// whether the victim's eyes were under water.
#[derive(Message, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tf2Blood {
    pub at: Vec3,
    pub toward: Vec3,
    pub under_water: bool,
}

/// Stains to trace behind a hit (8c): from its point along the shot, by
/// the gun's own per-hit damage in points.
#[derive(Message, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tf2Stains {
    pub at: Vec3,
    pub dir: Vec3,
    pub points: f32,
}

/// Whether a hit of `kind` while the attacker holds a Flame Thrower
/// (`flames`) bleeds (8a): not fire, not damage of no kind.
pub fn bleeds(kind: DamageKind, flames: bool) -> bool {
    !flames && !matches!(kind, DamageKind::Generic | DamageKind::Burn)
}

/// The server's: each landed TF2 damage's burst and stains.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bleed(
    looks: Res<BloodLooks>,
    mut damage: MessageReader<Damage>,
    mut shots: MessageReader<WeaponEvent>,
    mut blasts: MessageReader<Tf2Explosion>,
    bodies: Query<(&Transform, Option<&MovementState>, Option<&Intent>)>,
    characters: Query<(), With<Intent>>,
    holders: Query<&Inventory>,
    weapons: Query<(&Weapon, Option<&DamageEffect>, Has<super::flame::Tf2Flame>)>,
    teams: Query<&Team>,
    friendly_fire: Option<Res<FriendlyFire>>,
    water: Option<Res<MapWater>>,
    mut bursts: MessageWriter<Tf2Blood>,
    mut stains: MessageWriter<Tf2Stains>,
) {
    let blasts: Vec<Vec3> = blasts.read().map(|b| b.at).collect();
    // Each bullet or pellet on an enemy: stains by the gun's own damage
    // (8c), the pellets of one blast each theirs.
    for e in shots.read().filter(|e| e.shown()) {
        let (WeaponEventKind::Shot { from, to, hit: Some(hit), .. } | WeaponEventKind::ShotContinued { from, to, hit: Some(hit), .. }) = e.kind else {
            continue;
        };
        let Ok((w, Some(effect), _)) = weapons.get(e.weapon) else { continue };
        let teammates = matches!((teams.get(e.owner), teams.get(hit)), (Ok(a), Ok(b)) if a == b && a.0 != 0)
            && friendly_fire.as_deref().is_none_or(|f| f.0 == 0);
        if chimera_sim::weapon::impact::weapon_game(w.id) != Some(crate::game::GAME) || !characters.contains(hit) || hit == e.owner || teammates {
            continue;
        }
        stains.write(Tf2Stains { at: to, dir: (to - from).normalize_or_zero(), points: effect.amount * 100.0 });
    }
    let mut done: Vec<(Entity, Entity)> = Vec::new();
    for d in damage.read() {
        let Some(attacker) = d.attacker else { continue };
        let held = holders.get(attacker).ok().and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let gun = d.weapon.or(held.map(|(w, ..)| w.id));
        if gun.and_then(chimera_sim::weapon::impact::weapon_game) != Some(crate::game::GAME)
            || d.amount <= 0.0
            || !characters.contains(d.target)
            || refused_by_team(d, &teams, friendly_fire.as_deref())
            || !bleeds(d.kind, held.is_some_and(|(.., f)| f))
        {
            continue;
        }
        let Ok((victim, state, _)) = bodies.get(d.target) else { continue };
        let lower = Vec3::Y * looks.inflictor_lower * U;
        let attacker_at = bodies.get(attacker).ok();
        // A swing's stains (its damage is its own); bullets' are their
        // traces' (below).
        if d.kind == DamageKind::Melee {
            stains.write(Tf2Stains { at: d.point, dir: d.dir, points: d.amount * 100.0 });
        }
        if done.contains(&(attacker, d.target)) {
            continue;
        }
        done.push((attacker, d.target));
        let (at, inflictor) = match d.kind {
            DamageKind::Melee => {
                let Some((t, s, i)) = attacker_at else { continue };
                let eye = t.translation + s.map_or(Vec3::ZERO, |s| s.eye_offset);
                let aim = i.map_or(t.rotation, |i| aim_of(i.yaw, i.pitch));
                (eye + aim * Vec3::NEG_Z * looks.melee_reach * U, t.translation)
            }
            DamageKind::Blast | DamageKind::BlastSurface => {
                let Some(c) = blasts.iter().copied().min_by(|a, b| a.distance(victim.translation).total_cmp(&b.distance(victim.translation))) else {
                    continue;
                };
                (c, c)
            }
            _ => (d.point, attacker_at.map_or(d.point - d.dir, |(t, ..)| t.translation)),
        };
        let toward = (inflictor - lower - victim.translation).normalize_or(-d.dir);
        let eyes = victim.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
        bursts.write(Tf2Blood { at, toward, under_water: in_water(water.as_deref(), eyes) });
    }
}

/// Every machine that draws: bursts when the delay has passed, with this
/// viewer's spray; stains at once.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    time: Res<Time>,
    looks: Res<BloodLooks>,
    delay: Option<Res<LateDelay>>,
    settings: Option<Res<ImpactEffectSettings>>,
    eyes: Query<&GlobalTransform, (With<Camera3d>, Without<ViewModelCamera>, Without<SkyboxCamera>)>,
    mut bursts: MessageReader<Tf2Blood>,
    mut stain_hits: MessageReader<Tf2Stains>,
    mut held: Local<Vec<(f64, Tf2Blood)>>,
    mut painter: BloodPainter,
    mut start: MessageWriter<crate::map::psys::StartParticleSystem>,
) {
    let low = settings.as_ref().is_some_and(|s| s.violence_hblood == 0);
    for s in stain_hits.read() {
        if !low {
            painter.stains(s.at, s.dir, bleed_traces(s.points));
        }
    }
    let now = time.elapsed_secs_f64();
    let wait = delay.map_or(0.0, |d| d.0 as f64);
    held.extend(bursts.read().map(|b| (now + wait, *b)));
    let eye = eyes.iter().next().copied();
    let mut due = Vec::new();
    held.retain(|(t, b)| {
        let go = *t <= now + 1e-9;
        if go {
            due.push(*b);
        }
        !go
    });
    for b in due {
        use crate::map::psys::StartParticleSystem;
        start.write(StartParticleSystem::new(looks.burst(b.under_water, low), b.at, b.toward));
        if b.under_water {
            continue;
        }
        let (axis, distance) = match eye {
            Some(e) => {
                let distance = e.translation().distance(b.at) / U;
                let (k0, coin) = {
                    let r = &mut painter.rng().0;
                    (r.float(0.5, 1.5), r.random() < 0.5)
                };
                (looks.spray_axis(b.toward, e.forward().into(), e.right().into(), distance, k0, coin), distance)
            }
            None => (-b.toward, 0.0),
        };
        start.write(StartParticleSystem::new(looks.spray_name(distance, low), b.at, axis.normalize_or(-b.toward)));
    }
}

/// The server: bursts and stains to every client.
fn forward(
    mut bursts: MessageReader<Tf2Blood>,
    mut stains: MessageReader<Tf2Stains>,
    mut out_b: MessageWriter<bevy_replicon::prelude::ToClients<Tf2Blood>>,
    mut out_s: MessageWriter<bevy_replicon::prelude::ToClients<Tf2Stains>>,
) {
    use bevy_replicon::prelude::*;
    for b in bursts.read() {
        out_b.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: *b });
    }
    for s in stains.read() {
        out_s.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: *s });
    }
}

/// The body decals' TF2 rule here (effects.md 8b; drawn by Chimera's
/// `render::body_decals`): healing to full health (health rose and is at
/// least the max) removes them all (`ClearBodyDecals`). (That the ragdoll
/// keeps none is a class part, `movement::ClassParts`.)
fn body_decal_rules(
    health: Query<(Entity, &Health), (Changed<Health>, With<Tf2Class>)>,
    mut gone: RemovedComponents<Tf2Class>,
    mut seen: Local<HashMap<Entity, f32>>,
    mut clear: MessageWriter<ClearBodyDecals>,
) {
    for e in gone.read() {
        seen.remove(&e);
    }
    for (e, h) in &health {
        let before = seen.insert(e, h.current);
        if before.is_some_and(|b| h.current > b) && h.current >= h.max {
            clear.write(ClearBodyDecals(e));
        }
    }
}

fn register(app: &mut App) {
    use crate::core::NetRole;
    use bevy_replicon::prelude::*;
    app.add_server_message::<Tf2Blood>(Channel::Unreliable)
        .add_server_message::<Tf2Stains>(Channel::Unreliable)
        .add_systems(PostUpdate, forward.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)));
}

/// TF2's blood (from `looks::add`).
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.init_resource::<BloodLooks>()
        .add_message::<Tf2Blood>()
        .add_message::<Tf2Stains>()
        .add_message::<Tf2Explosion>()
        .add_systems(
            FixedUpdate,
            bleed.after(chimera_core::core::SimSet::Weapons).run_if(chimera_core::core::authoritative),
        )
        .add_message::<ClearBodyDecals>()
        .add_systems(
            Update,
            (
                draw.run_if(not(resource_exists::<chimera_net::dedicated::Serving>))
                    .before(crate::map::psys::StartedSystemsRun),
                body_decal_rules.run_if(not(resource_exists::<chimera_net::dedicated::Serving>)),
            ),
        )
        .net_part("tf2_blood", register);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// effects.md 8's test cases: the systems by distance, water and
    /// violence; fire and damage of no kind don't bleed.
    #[test]
    fn blood_systems_follow_the_spec_cases() {
        let l = BloodLooks::default();
        assert_eq!(l.burst(false, false), "blood_impact_red_01");
        assert_eq!(l.spray_name(180.0, false), "blood_spray_red_01");
        assert_eq!(l.spray_name(500.0, false), "blood_spray_red_01_far");
        assert_eq!(l.burst(true, false), "water_blood_impact_red_01");
        assert_eq!((l.burst(false, true), l.spray_name(180.0, true)), ("lowV_blood_impact_red_01", "lowV_blood_spray_red_01"));
        assert!(bleeds(DamageKind::Bullet, false) && bleeds(DamageKind::Blast, false) && bleeds(DamageKind::Melee, false));
        assert!(!bleeds(DamageKind::Burn, false) && !bleeds(DamageKind::Bullet, true) && !bleeds(DamageKind::Generic, false));
        // A spray across the view stays; one along it is pushed sideways.
        let (f, r) = (Vec3::NEG_Z, Vec3::X);
        let across = Vec3::X;
        assert_eq!(l.spray_axis(across, f, r, 180.0, 1.0, true), -across);
        let along = l.spray_axis(Vec3::Z, f, r, 180.0, 1.0, true);
        assert!((along - Vec3::new(1.0 + 0.25 * 180.0 / 512.0, 0.0, -1.0)).length() < 1e-5, "{along}");
        let far_left = l.spray_axis(Vec3::new(0.1, 0.0, 1.0), f, r, 1024.0, 1.0, true);
        assert!(far_left.x < 0.0, "from 512 it leans its own way: {far_left}");
    }
}
