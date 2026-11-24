//! Batch 3b's odd ones (specs/tf2/batch3b_mechanics.md 10, 11): the Cow
//! Mangler 5000's charged shot (`tf2:mangler`, `ManglerParams`, the part
//! `Tf2Mangler`) and the Razorback's backstab block. The Hitman's
//! Heatmaker's focus is `meters`' (`MeterKind::Focus`, the scope kept up
//! in `weapons::sniper`).
//!
//! - **The Cow Mangler** (its base data/weapons/particle_cannon.toml: an
//!   energy cell of 4 shots, 1100 units/s rockets, 5 back at a time):
//!   attack2 with the cell full starts a charge (`mangler`, predicted,
//!   before the frame): no shot meanwhile, its holder's max speed at most
//!   `speed` (80, `speed_cap`); `charge` s (2.0) later it fires once and
//!   the cell is empty. That rocket (`Charged`, marked on the server as it
//!   launches: `mark_charged`) mini-crits what its blast hits and sets
//!   each burning `burn` s (6; `charged_burns`). Its crits are mini-crits
//!   (`ItemParams::crits_mini`, `effects`) and an airblast doesn't turn its
//!   shots (`ItemParams::no_reflect`, `weapons::airblast`).
//! - **The Razorback** (`backstab_block`, the server's, before the hits'
//!   crits): a worn item; a backstab on its wearer while it is ready does
//!   nothing, breaks it (`Razorbacks`) and keeps the Spy from attacking for
//!   `RAZORBACK_STALL` s (2.0); it is ready again `RAZORBACK_RECHARGE` s
//!   (30) later.
//!
//! Known shortcuts (provisional): the Razorback's block returns when its
//! meter is full again (the spec's open question: a script refill didn't
//! restore it), it has no sound, look or HUD meter; the Mangler's charge
//! has no sound or look, its shots' 20 % on buildings and disabling (no
//! buildings) aren't built, the charged shot's burn is the blast's
//! victims' of that tick.

use bevy::{platform::collections::HashMap, prelude::*};
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    Inventory, Magazine, NetPart, Weapon, WeaponState,
    projectile::Flyer,
};

use super::mods::Tf2Item;
use crate::core::{Damage, DamageTaken, Health, Intent, NetRole, SimClock};
use crate::game::weapons::{flame::Burning, melee::BACKSTAB};

/// The hook.
pub const MANGLER_HOOK: &str = "tf2:mangler";
/// The Razorback (§10): the Spy's stall and the recharge (s).
pub const RAZORBACK_STALL: f64 = 2.0;
pub const RAZORBACK_RECHARGE: f64 = 30.0;

/// `tf2:mangler`'s parameters (s, units/s).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ManglerParams {
    pub charge: f32,
    pub speed: f32,
    pub burn: f32,
}

impl Default for ManglerParams {
    fn default() -> Self {
        Self { charge: 2.0, speed: 80.0, burn: 6.0 }
    }
}

/// The charge's state (predicted): its start (negative: none).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManglerState {
    pub since: f64,
    pub prev_alt: bool,
}

impl Default for ManglerState {
    fn default() -> Self {
        Self { since: -1.0, prev_alt: false }
    }
}

/// The weapon part.
#[derive(Component, Clone, Debug)]
pub struct Tf2Mangler {
    pub params: ManglerParams,
    pub state: ManglerState,
}

impl NetPart for Tf2Mangler {
    type Net = ManglerState;
    fn net(&self) -> ManglerState {
        self.state
    }
    fn set_net(&mut self, net: ManglerState) {
        self.state = net;
    }
}

/// A charged rocket (mini-crits, ignites).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Charged;

/// Charged shots fired this tick (owner, weapon) and charged blasts
/// (owner, weapon, time): the server's.
#[derive(Clone, Debug, Default)]
pub struct ChargedShots {
    pub fired: Vec<(Entity, &'static str)>,
    pub blasts: Vec<(Entity, &'static str, f64)>,
}

/// The max speed a charging Mangler in `inv` allows (units/s), if any.
pub fn speed_cap(inv: &Inventory, manglers: &Query<&Tf2Mangler>) -> Option<f32> {
    inv.active.and_then(|w| manglers.get(w).ok()).filter(|m| m.state.since >= 0.0).map(|m| m.params.speed)
}

/// The charge (module doc), predicted, before the frame.
#[allow(clippy::type_complexity)]
pub fn mangler(
    mut owners: Query<(Entity, &Inventory, &mut Intent, Option<&Health>)>,
    mut guns: Query<(&mut Tf2Mangler, &mut WeaponState, &mut Magazine, &Weapon)>,
    clock: Res<SimClock>,
    role: Option<Res<NetRole>>,
    first: Res<crate::core::FirstTimePredicted>,
    mut state: ResMut<super::ItemState>,
) {
    let shots = &mut state.charged;
    let now = clock.now;
    for (owner, inv, mut intent, health) in &mut owners {
        let Some(w) = inv.active else { continue };
        let Ok((mut m, mut st, mut mag, weapon)) = guns.get_mut(w) else { continue };
        let mut s = m.state;
        let press = intent.secondary && !s.prev_alt;
        s.prev_alt = intent.secondary;
        if health.is_some_and(|h| h.current <= 0.0) {
            s.since = -1.0;
        } else if s.since >= 0.0 {
            if now + 1e-6 >= s.since + m.params.charge as f64 {
                // The charged shot: the whole cell.
                s.since = -1.0;
                mag.clip = 1;
                st.next_primary = st.next_primary.min(now);
                intent.fire = true;
                if first.0 && super::consumables::serving(role.as_deref()) {
                    shots.fired.push((owner, weapon.id));
                }
            } else {
                intent.fire = false;
            }
        } else if press && mag.clip >= mag.size && mag.size > 0 && st.next_primary <= now + 1e-6 && inv.ready(now) {
            s.since = now;
            intent.fire = false;
        }
        if m.state != s {
            m.state = s;
        }
    }
}

/// The charged shot's rocket marked as it launches (the server's).
pub fn mark_charged(flyers: Query<(Entity, &Flyer), Added<Flyer>>, mut state: ResMut<super::ItemState>, mut commands: Commands) {
    let shots = &mut state.charged;
    if shots.fired.is_empty() {
        return;
    }
    for (e, f) in &flyers {
        if let Some(i) = shots.fired.iter().position(|(o, w)| f.owner == Some(*o) && f.weapon == *w) {
            shots.fired.remove(i);
            commands.entity(e).insert(Charged);
        }
    }
    shots.fired.clear();
}

/// A charged blast's victims set burning (the server's, after the damage).
pub fn charged_burns(
    mut taken: MessageReader<DamageTaken>,
    mut state: ResMut<super::ItemState>,
    manglers: Query<&Tf2Mangler>,
    holders: Query<&Inventory>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    let shots = &mut state.charged;
    shots.blasts.retain(|b| b.2 + 1e-6 >= now);
    if shots.blasts.is_empty() {
        taken.clear();
        return;
    }
    for t in taken.read() {
        let d = &t.damage;
        let Some(a) = d.attacker.filter(|a| *a != d.target) else { continue };
        let Some(w) = d.weapon else { continue };
        if !d.kind.is_blast() || !shots.blasts.iter().any(|b| b.0 == a && b.1 == w) {
            continue;
        }
        let secs = holders
            .get(a)
            .ok()
            .and_then(|inv| inv.weapons.iter().find_map(|x| manglers.get(*x).ok()))
            .map_or(ManglerParams::default().burn, |m| m.params.burn);
        let fire = crate::game::weapons::flame::stock_params();
        commands.entity(d.target).insert(Burning { left: secs, ..crate::game::weapons::flame::ignited(now, a, w, &fire) });
    }
}

/// Each Razorback's break time (weapon entity), the server's.
#[derive(Clone, Debug, Default)]
pub struct Razorbacks(pub HashMap<Entity, f64>);

/// The Razorback's block (module doc).
pub fn backstab_block(
    mut damage: MessageMutator<Damage>,
    mut holders: Query<&mut Inventory>,
    items: Query<&Tf2Item>,
    mut states: Query<&mut WeaponState>,
    mut state: ResMut<super::ItemState>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    let broken = &mut state.razorbacks;
    for d in damage.read() {
        if d.weapon != Some(BACKSTAB) || d.amount <= 0.0 {
            continue;
        }
        let Ok(inv) = holders.get(d.target) else { continue };
        let ready = inv
            .weapons
            .iter()
            .copied()
            .find(|w| items.get(*w).is_ok_and(|i| i.0.block_backstab) && broken.0.get(w).is_none_or(|t| now + 1e-6 >= t + RAZORBACK_RECHARGE));
        let Some(shield) = ready else { continue };
        d.amount = 0.0;
        broken.0.insert(shield, now);
        if let Some(a) = d.attacker
            && let Ok(mut spy) = holders.get_mut(a)
        {
            let until = now + RAZORBACK_STALL;
            spy.next_attack = spy.next_attack.max(until);
            for w in spy.weapons.clone() {
                if let Ok(mut st) = states.get_mut(w) {
                    st.next_primary = st.next_primary.max(until);
                    st.next_secondary = st.next_secondary.max(until);
                }
            }
        }
    }
}

/// The hook, the part and the systems (`mods::add` calls it).
pub fn add(app: &mut App) {
    use chimera_sim::weapon::{CarriedAppExt, WeaponFrame, data::WeaponDataAppExt, projectile::FlyerSet};
    app.register_weapon_hook::<ManglerParams>(MANGLER_HOOK, |e, _, p| {
            e.insert(Tf2Mangler { params: p.clone(), state: ManglerState::default() });
        })
        .predicted_part::<Tf2Mangler>()
        .add_systems(crate::core::Predict::Weapons, mangler.before(WeaponFrame).before(crate::game::weapons::airblast::fire))
        .add_systems(
            FixedUpdate,
            (
                mark_charged.after(FlyerSet::Launch).before(FlyerSet::Touch),
                backstab_block.after(crate::core::SimSet::Weapons).before(super::effects::crits),
                charged_burns.after(crate::core::apply_damage),
            )
                .run_if(crate::core::authoritative),
        );
}
