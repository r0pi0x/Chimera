//! Crit boosts on the held weapon (specs/tf2/effects.md 14), on every
//! machine that draws, each deciding for each player it has:
//!
//! - **The boost** (`CritBoosted`: any crit-boost condition, the server's,
//!   sent to every client) shows when the player has it, is alive and
//!   holds a weapon that can be boosted (`boostable`: any of TF2's).
//! - On the change to showing, `Weapon_General.CritPower` (`CRIT_POWER`)
//!   starts at once at full volume on the player (the sound follows him;
//!   its WAV loops whole) and the glow `critgun_weaponmodel_red`/`_blu`
//!   (`GLOW`, his team's) on the weapon drawn for him (its muzzle on the
//!   view model or the held model, else ahead of the eye; then on its
//!   model's hitboxes, below), tagged with the
//!   weapon. A switch between two boostable weapons keeps the loop going
//!   and moves the glow; the boost ending, a death or a weapon that can't
//!   be boosted stop both at once (`StopSound::local`,
//!   `StopParticleSystems`). Random crits never play it.
//!
//! The Flame Thrower's crit loop swap is `flame::sounds`'s (a boosted
//! Pyro's every burst is critical).
//!
//! The glow stands on a model (14.1, `ParticleModel` on the weapon each
//! frame, `Muzzles::glow_model`): for the local player in first person
//! the arms view model's hitboxes (`c_<class>_arms.mdl`'s set, drawn in
//! the view-model pass), for others their held weapon's world model's
//! (its `$hitbox` attachments); its particles placed on random hitboxes
//! (best of 6 with the (1, 1, 1) bias) and kept on them.
//!
//! Known shortcuts: nothing in ours grants a boost yet (no Kritzkrieg,
//! first blood or round-win crits: a mode or a test sets `CritBoosted`);
//! there is no cloak or disguise, so those rules don't apply; the glow
//! is on the held weapon's boxes at their reference pose relative to the
//! bone that holds it (its own bones' motion not followed); control
//! point 0 for others is the player's origin (the weapon entity's in
//! Source).

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_core::core::{Health, Team};
use chimera_sim::weapon::{Inventory, Weapon, aim_of};

use super::looks::{Muzzles, ahead_of};
use crate::map::{
    PlaySound, StopSound,
    psys::{ParticleModel, StartParticleSystem, StopParticleSystems},
};

/// The boost's loop (effects.md 14).
pub const CRIT_POWER: &str = "tf2:Weapon_General.CritPower";
/// The glow on the boosted weapon, `_red` or `_blu` by team.
pub const GLOW: &str = "critgun_weaponmodel";
/// Source's static sound channel (the loop's).
pub const CHAN_STATIC: u8 = 6;

/// On a player with any crit-boost condition (the server's; replicated).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CritBoosted;

/// Whether the weapon `id` can be crit-boosted: any of TF2's.
pub fn boostable(id: &'static str) -> bool {
    chimera_sim::weapon::impact::weapon_game(id) == Some(crate::game::GAME)
}

/// The glow's system for a team.
pub fn glow_name(team: Option<Team>) -> String {
    let blu = super::super::bodies::skin_of(team) == 1;
    format!("{GLOW}_{}", if blu { "blu" } else { "red" })
}

/// Each player's boost loop and glow on this machine: by player, the
/// weapon its glow is on.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn boost_looks(
    players: Query<(Entity, &Inventory, &Transform, Option<&Team>, Option<&Health>, Has<CritBoosted>, Option<&chimera_core::core::MovementState>, Option<&chimera_core::core::Intent>)>,
    boosted_players: Query<Entity, With<CritBoosted>>,
    weapons: Query<&Weapon>,
    muzzles: Muzzles,
    mut shown: Local<BTreeMap<Entity, Entity>>,
    mut play: MessageWriter<PlaySound>,
    mut stops: MessageWriter<StopSound>,
    mut start: MessageWriter<StartParticleSystem>,
    mut stop: MessageWriter<StopParticleSystems>,
    (mut hosts, mut commands): (Query<&mut ParticleModel>, Commands),
) {
    let end = |p: Entity, w: Entity, stops: &mut MessageWriter<StopSound>, stop: &mut MessageWriter<StopParticleSystems>| {
        stops.write(StopSound { by: None, source: p, entry: CRIT_POWER.to_string(), local: true });
        stop.write(StopParticleSystems { tag: w });
    };
    // Players gone or no longer boosted: silent.
    shown.retain(|p, w| {
        let keep = players.get(*p).is_ok_and(|x| x.5);
        if !keep {
            end(*p, *w, &mut stops, &mut stop);
        }
        keep
    });
    // Only the boosted (a query over every player each frame costs per
    // character archetype: party_kit's archetypes test).
    for (p, inv, at, team, health, boosted, state, intent) in boosted_players.iter().filter_map(|p| players.get(p).ok()) {
        let alive = health.is_none_or(|h| h.current > 0.0);
        let wanted = inv
            .active
            .filter(|w| boosted && alive && weapons.get(*w).is_ok_and(|w| boostable(w.id)));
        let now = shown.get(&p).copied();
        if now == wanted {
            continue;
        }
        match (now, wanted) {
            (Some(old), None) => {
                end(p, old, &mut stops, &mut stop);
                shown.remove(&p);
                continue;
            }
            // Another boostable weapon: the loop goes on, the glow moves.
            (Some(old), Some(_)) => {
                stop.write(StopParticleSystems { tag: old });
            }
            (None, Some(_)) => {
                play.write(PlaySound {
                    entry: CRIT_POWER.to_string(),
                    at: Some(at.translation),
                    volume: None,
                    pitch: None,
                    source: Some(p),
                    channel: Some(CHAN_STATIC),
                });
            }
            (None, None) => continue,
        }
        let Some(w) = wanted else { continue };
        shown.insert(p, w);
        let aim = intent.map_or(at.rotation, |i| aim_of(i.yaw, i.pitch));
        let forward = aim * Vec3::NEG_Z;
        let name = glow_name(team.copied());
        let mut s = match muzzles.placed(p) {
            Some(m) => muzzles.start_on(p, &m, &name, forward),
            None => {
                let eye = at.translation + state.map_or(Vec3::ZERO, |s| s.eye_offset);
                StartParticleSystem::new(name, ahead_of(eye, aim), forward)
            }
        };
        // On a model's hitboxes, its point that model's origin (14.1).
        if muzzles.glow_model(p).is_some() {
            s.follow = None;
        }
        start.write(StartParticleSystem { tag: Some(w), ..s });
    }
    // Each glow's host model this frame (effects.md 14.1): the arms view
    // model for the local player, the held weapon for others.
    for (&p, &w) in shown.iter() {
        let Some(m) = muzzles.glow_model(p) else { continue };
        match hosts.get_mut(w) {
            Ok(mut h) => {
                if *h != m {
                    *h = m;
                }
            }
            Err(_) => {
                commands.entity(w).insert(m);
            }
        }
    }
}

fn register(app: &mut App) {
    use bevy_replicon::prelude::*;
    app.replicate::<CritBoosted>();
}

/// The boost's looks on every machine that draws, `CritBoosted` on the
/// network.
pub fn add(app: &mut App) {
    use chimera_net::net::parts::NetAppExt;
    app.add_message::<PlaySound>()
        .add_message::<StopSound>()
        .add_message::<StartParticleSystem>()
        .add_message::<StopParticleSystems>()
        .add_systems(
            Update,
            boost_looks
                .after(crate::map::attached::EyePlaced)
                .after(crate::map::DriveAnimation)
                .after(crate::map::attached::MarksPlaced)
                .before(crate::map::psys::StartedSystemsRun),
        )
        .net_part("tf2_crit_boost", register);
}
