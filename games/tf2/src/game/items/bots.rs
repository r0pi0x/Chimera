//! Bots using TF2's keyed items (docs/plans/active/rung-passives.md, P3):
//! a bot's brain (Combat Arms', in the party) fights with its rung's gun
//! and knows nothing of TF2; these hooks (`use_items`, Chimera's
//! `core::BotItems`, the server's) press the keys the items take after
//! the brain wrote its own, from what it has in mind (`core::BotFocus`:
//! its target, whether it sees it). Each is one simple rule:
//!
//! - **A shield** (`shields::Tf2Shield`): a full meter and a target in
//!   sight within `CHARGE_REACH` units ahead of its aim (`CHARGE_AHEAD`):
//!   attack2, so it charges at him (the charge holds the brain's aim as
//!   its heading).
//! - **A watch** (`spy::Tf2Cloak`): hurt (under `HURT` of its max health)
//!   with an enemy about and `CLOAK_FROM` of meter or more: attack2, cloak;
//!   the Dead Ringer instead arms with a full meter whenever it sees a
//!   target, so the next hit feigns. Cloaked (either), it backs away from
//!   its target and decloaks once it has been hidden `HIDE_FOR` s with
//!   nobody in sight (the watch decloaks by itself on an empty meter).
//! - **The B.A.S.E. Jumper** (`kit::Tf2Chute`): falling faster than
//!   `CHUTE_FALL` units/s (a long fall: about 150 units down), not yet
//!   opened this air time: jump, so it opens.
//! - **The Thermal Thruster** (`kit::Tf2Thruster`, carried): a target
//!   farther than `THRUST_FROM` units, a charge in the meter, on the
//!   ground: it draws the thruster (its slot) and fires once drawn, then
//!   draws its gun back as soon as the launch is under way.
//!
//! - **Utility items** (P5, carried, used with fire: `consumables::
//!   Tf2Consumable`, a banner's `meters::Tf2Meter`): each is drawn, fired
//!   once and put away for the gun again (`use_once`) when its rule says
//!   so, and only with a full meter: a jar at a target in sight within
//!   `JAR_REACH` units; a lunch hurt with nobody in sight; Bonk hurt with a
//!   target in sight, Crit-a-Cola with a target in sight; a banner with a
//!   full rage and a target in sight. Eating and drinking draw the gun back
//!   themselves after the taunt.
//!
//! Known shortcuts: the rules are ours (TF2's bots never use these items);
//! no look at the ground before a chute or a launch (a launch may fly off
//! a ledge); a cloaked bot backs away blind (the brain's own path isn't
//! asked); a charge doesn't swing a melee weapon at its end (the rung's
//! gun keeps firing as the brain fires it).

use bevy::prelude::*;

use chimera_sim::weapon::{Inventory, Unselectable};

use super::{
    consumables::{Kind as UseKind, Tf2Consumable},
    kit::{Tf2Chute, Tf2Thruster},
    meters::{MeterKind, Tf2Meter},
    shields::{self, Tf2Shield},
    spy::{CLOAK_LOCK, CloakKind, Tf2Cloak},
};
use crate::core::{Bot, BotFocus, Health, Intent, MovementState, SimClock, Velocity};
use crate::game::weapons::damage::U;

/// A charge starts at a target this close (units) ...
pub const CHARGE_REACH: f32 = 600.0;
/// ... this near the aim (cosine of the angle).
pub const CHARGE_AHEAD: f32 = 0.95;
/// Hurt: under this share of its max health.
pub const HURT: f32 = 0.5;
/// The least meter to cloak with.
pub const CLOAK_FROM: f32 = 40.0;
/// Cloaked this long (s) with nobody in sight: decloak.
pub const HIDE_FOR: f64 = 3.0;
/// The fall (units/s, down) that opens the chute.
pub const CHUTE_FALL: f32 = 550.0;
/// The thruster closes on a target farther than this (units).
pub const THRUST_FROM: f32 = 900.0;
/// A jar is thrown at a target this close (units; it flies about 1000
/// forward) ...
pub const JAR_REACH: f32 = 800.0;
/// ... and no closer than this (its splash would reach the thrower's
/// own place).
pub const JAR_FROM: f32 = 150.0;

/// The bots' keyed items (module doc).
#[allow(clippy::type_complexity)]
pub fn use_items(
    mut bots: Query<
        (
            Entity,
            &BotFocus,
            &mut Intent,
            &mut Inventory,
            &Transform,
            Option<&Health>,
            Option<&Velocity>,
            Option<&MovementState>,
        ),
        With<Bot>,
    >,
    places: Query<&Transform>,
    shields: Query<&Tf2Shield>,
    cloaks: Query<&Tf2Cloak>,
    chutes: Query<&Tf2Chute>,
    packs: Query<&Tf2Thruster>,
    (uses, rages): (Query<&Tf2Consumable>, Query<&Tf2Meter>),
    unselectable: Query<(), With<Unselectable>>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (_, focus, mut intent, mut inv, at, health, velocity, movement) in &mut bots {
        if health.is_some_and(|h| h.current <= 0.0) || inv.weapons.is_empty() {
            continue;
        }
        let target = focus
            .target
            .and_then(|t| places.get(t).ok())
            .map(|t| (t.translation - at.translation) / U);
        let aim = Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0) * Vec3::NEG_Z;
        let hurt = health.is_some_and(|h| h.max > 0.0 && h.current / h.max < HURT);
        let grounded = movement.is_none_or(|m| m.on_ground);
        for &w in &inv.weapons.clone() {
            if let Ok(s) = shields.get(w) {
                let st = s.state;
                if !st.charging
                    && st.meter >= shields::FULL - 1e-3
                    && focus.sees
                    && let Some(to) = target
                    && to.length() <= CHARGE_REACH
                    && to.normalize_or_zero().dot(aim) >= CHARGE_AHEAD
                {
                    intent.secondary = true;
                }
            } else if let Ok(c) = cloaks.get(w) {
                let st = c.state;
                if st.cloaked {
                    // Back away from the target; decloak when hidden long
                    // enough with nobody in sight.
                    if target.is_some() {
                        intent.move_axis = Vec2::new(0.0, -1.0);
                    }
                    if !focus.sees && now >= st.changed + HIDE_FOR.max(CLOAK_LOCK) {
                        intent.secondary = true;
                    }
                } else if c.params.kind == CloakKind::Feign {
                    if !st.armed && st.meter >= super::spy::FULL - 1e-3 && focus.sees {
                        intent.secondary = true;
                    }
                } else if hurt && target.is_some() && st.meter >= CLOAK_FROM {
                    intent.secondary = true;
                }
            } else if let Ok(c) = chutes.get(w) {
                let falling = velocity.map_or(0.0, |v| -v.0.y / U);
                if !grounded && !c.state.deployed && falling > CHUTE_FALL {
                    intent.jump = true;
                }
            } else if let Ok(p) = packs.get(w) {
                let st = p.state;
                let busy = st.flying || st.launch_at >= 0.0;
                if inv.active == Some(w) {
                    if busy || st.meter < p.params.charge - 1e-3 {
                        // Launched (or empty): the gun back.
                        let back = inv.last.filter(|l| *l != w && inv.weapons.contains(l)).or_else(|| {
                            inv.weapons
                                .iter()
                                .copied()
                                .find(|x| *x != w && !unselectable.contains(*x))
                        });
                        if st.flying && back.is_some() && inv.wanted != back {
                            inv.wanted = back;
                        }
                        intent.fire = false;
                    } else {
                        // Drawn: one press once ready.
                        intent.fire = !st.prev_fire && inv.ready(now);
                    }
                } else if !busy
                    && grounded
                    && st.meter >= p.params.charge - 1e-3
                    && target.is_some_and(|to| to.length() > THRUST_FROM)
                    && inv.wanted != Some(w)
                {
                    inv.wanted = Some(w);
                }
            } else if let Ok(u) = uses.get(w) {
                let st = u.state;
                if st.started.is_some() {
                    // Eating or drinking: the taunt draws the gun back.
                    continue;
                }
                let full = st.meter >= 1.0 - 1e-3;
                let want = full
                    && match u.params.kind {
                        UseKind::Jar => focus.sees && target.is_some_and(|to| (JAR_FROM..=JAR_REACH).contains(&to.length())),
                        UseKind::Lunch => hurt && !focus.sees,
                        UseKind::Drink => focus.sees && (hurt || !u.params.phase),
                    };
                use_once(&mut inv, &mut intent, w, want, full, st.prev_fire, now, &unselectable);
            } else if let Ok(m) = rages.get(w)
                && m.params.kind == MeterKind::Rage
                && m.params.banner > 0
            {
                let full = m.state.value >= m.params.max - 1e-3 && !m.active(now);
                use_once(&mut inv, &mut intent, w, full && focus.sees, full, m.state.prev_fire, now, &unselectable);
            }
        }
    }
}

/// A utility item `w` used once (module doc): drawn when `want`, fired
/// once drawn while `ready` (its meter full), then the gun drawn back.
#[allow(clippy::too_many_arguments)]
fn use_once(inv: &mut Inventory, intent: &mut Intent, w: Entity, want: bool, ready: bool, prev_fire: bool, now: f64, unselectable: &Query<(), With<Unselectable>>) {
    if inv.active == Some(w) {
        if ready {
            intent.fire = !prev_fire && inv.ready(now);
        } else {
            // Used (or empty): the gun back.
            intent.fire = false;
            let back = inv
                .last
                .filter(|l| *l != w && inv.weapons.contains(l))
                .or_else(|| inv.weapons.iter().copied().find(|x| *x != w && !unselectable.contains(*x)));
            if back.is_some() && inv.wanted != back {
                inv.wanted = back;
            }
        }
    } else if want && inv.wanted != Some(w) {
        inv.wanted = Some(w);
    }
}

/// The system (`mods::add` calls it).
pub fn add(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        use_items
            .in_set(crate::core::BotItems)
            .before(crate::core::SimSet::Rules)
            .before(chimera_sim::weapon::SelectWeapons)
            .run_if(crate::core::authoritative),
    );
}
