//! The movement's root: its systems as one plugin (`SourceMovementPlugin`:
//! the step in `core::Predict::Movement`, the physics shadow, the
//! push-away, the drowning, the ladder lines), the set games order their
//! own per-tick choices against (`SourceMovementStep`), and a movement's
//! registration (`QuakeMovementAppExt::quake_movement`: its id, config,
//! console variables by list and namespace, its envelope and its seat).
//! Engine-neutral: what an engine adds (Source's cvars, its key impulse,
//! its ladder entities) is its wrapper's (`source_movement`'s
//! `SourceMovementAppExt`).

use bevy::prelude::*;

use crate::{
    core::PredictedAppExt,
    slots::RegisterSlots,
    source_movement::{
        config::{SourceMovementConfig, SourceMovementConfigs},
        drown,
        mover::stuck::{footing, seat},
        pushaway, shadow,
        state::SourceMovement,
        step::step,
        useable_ladders::{self, EntityLadders, UseableLadders},
    },
};

/// The movement's systems, once per app (`QuakeMovementAppExt::
/// quake_movement` adds it): the step, the physics shadow, the push-away,
/// the drowning, the map's ladder lines, and the configs by movement id
/// (`SourceMovementConfigs`).
pub struct SourceMovementPlugin;

impl Plugin for SourceMovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SourceMovementConfigs>()
            .init_resource::<UseableLadders>()
            .init_resource::<EntityLadders>()
            .add_message::<crate::map::PlaySound>()
            .add_message::<crate::map::entities::FireEntityOutput>()
            .add_systems(First, useable_ladders::build_ladders)
            .add_systems(crate::core::Predict::Movement, step.in_set(SourceMovementStep))
            .predicted_net::<SourceMovement>()
            .add_plugins((pushaway::PushAwayPlugin, shadow::ShadowPlugin));
        app.add_plugins(drown::DrowningPlugin);
    }
}

/// The movement step: a game's own per-tick choices (HL2:DM's speed
/// selection, `MoveSpeed`) go before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceMovementStep;

/// Register a movement on the shared movement.
pub trait QuakeMovementAppExt {
    /// Movement `id` on the shared movement (`SourceMovementPlugin`, added
    /// once), with its `config` (one set for `id` before, by
    /// `SourceMovementConfigs::set`, stays) and the console variables of
    /// `cvars` (lists of (name, help): `config::QUAKE_CVARS` and the
    /// engine's own; each name one `SourceMovementConfig::set_cvar`
    /// knows), under `namespace` (`hl2dm:sv_gravity`; None: bare names).
    fn quake_movement(
        &mut self,
        id: &'static str,
        config: SourceMovementConfig,
        cvars: &[&'static [(&'static str, &'static str)]],
        namespace: Option<&'static str>,
    ) -> &mut Self;
}

impl QuakeMovementAppExt for App {
    fn quake_movement(
        &mut self,
        id: &'static str,
        config: SourceMovementConfig,
        cvars: &[&'static [(&'static str, &'static str)]],
        namespace: Option<&'static str>,
    ) -> &mut Self {
        if !self.is_plugin_added::<SourceMovementPlugin>() {
            self.add_plugins(SourceMovementPlugin);
        }
        let defaults = config;
        {
            let mut all = self.world_mut().get_resource_or_init::<SourceMovementConfigs>();
            if all.get(id).is_none() {
                all.0.push((id, defaults.clone()));
            }
        }
        for (name, help) in cvars.iter().flat_map(|list| list.iter()) {
            let name = *name;
            let full = match namespace {
                Some(ns) => format!("{ns}:{name}"),
                None => name.to_string(),
            };
            crate::console::ConsoleAppExt::console_cvar(
                self,
                &full,
                help,
                &defaults.get_cvar(name).unwrap_or_default(),
                move |w| w.get_resource::<SourceMovementConfigs>().and_then(|c| c.get(id)).and_then(|c| c.get_cvar(name)),
                move |w, v| {
                    w.get_resource_mut::<SourceMovementConfigs>()
                        .and_then(|c| c.into_inner().get_mut(id).map(|c| c.set_cvar(name, v)))
                        .ok_or("no Source movement")?
                },
            );
        }
        // The game's character's body, the envelope its maps carry
        // (`chimera_core::envelope`): the box's half width, heights, step.
        if let Some(game) = chimera_core::core::movement_game(id) {
            use chimera_core::envelope::MovementEnvelopeAppExt;
            self.add_movement_envelope(game, defaults.envelope());
        }
        self.register_movement::<SourceMovement>(id).seat_movement(id, footing, seat)
    }
}
