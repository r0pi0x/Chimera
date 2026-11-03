//! The shotgun's pump over the shared frame (specs/hl2dm/weapons.md 7.4),
//! `hl2dm:shotgun` (`Hl2Shotgun`): every shot (primary or the double,
//! `weapon::AltShot`) leaves a pump pending; once the shot's sequence is
//! over (the next primary passed) the pump plays: `pump_sound`, the next
//! attack (player and weapon) `pump` seconds on, the view model's
//! `Action("pump")`. While a pump is pending the reload waits for it (the
//! reload key is remembered and its reload starts after the pump). A
//! shotgun put away with a pump pending keeps it: after the next draw it
//! pumps before it can fire. Dry fire waits its sequence's `dry`
//! seconds. The rest is the frame's: shells one at a time, each in as its
//! insert starts (`ShellReload::insert_first`), a held attack ending the
//! reload, an empty tube reloading by itself (`Magazine::auto_reload`).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{Inventory, NetPart, WeaponEvent, WeaponEventKind, WeaponState};

use crate::{
    core::{FirstTimePredicted, Intent, SimClock},
    map::PlaySound,
};

/// `hl2dm:shotgun`'s parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShotgunParams {
    pub pump: f32,
    pub pump_sound: String,
    pub dry: f32,
}

/// The pump's state.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Shotgun {
    pub params: ShotgunParams,
    pub pump_pending: bool,
    pub reload_after_pump: bool,
}

impl NetPart for Hl2Shotgun {
    type Net = (bool, bool);
    fn net(&self) -> Self::Net {
        (self.pump_pending, self.reload_after_pump)
    }
    fn set_net(&mut self, (pump, reload): Self::Net) {
        self.pump_pending = pump;
        self.reload_after_pump = reload;
    }
}

/// The view-model action of the pump (`WeaponEventKind::Action`).
pub const PUMP: &str = "pump";

/// Before the weapon frame: the pending pump plays once the shot is over.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<(Entity, &Intent, &mut Inventory, &Transform), Without<crate::map::interp::NetDrawn>>,
    mut shotguns: Query<(&mut Hl2Shotgun, &mut WeaponState)>,
    mut events: MessageWriter<WeaponEvent>,
    mut play: MessageWriter<PlaySound>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
) {
    let now = clock.now;
    let due = now + 1e-5;
    for (owner, intent, mut inv, at) in &mut owners {
        let Some(active) = inv.active else { continue };
        let Ok((mut gun, mut st)) = shotguns.get_mut(active) else { continue };
        if !gun.pump_pending {
            if st.reload_held {
                st.reload_held = false;
            }
            continue;
        }
        if intent.reload {
            gun.reload_after_pump = true;
        }
        st.reload_held = true;
        if due < inv.next_attack || st.next_primary > due {
            continue;
        }
        gun.pump_pending = false;
        gun.reload_after_pump = false;
        st.reload_held = false;
        let end = now + gun.params.pump as f64;
        st.next_primary = end;
        st.next_secondary = end;
        inv.next_attack = end;
        if first.0 {
            play.write(PlaySound {
                pitch: None,
                entry: gun.params.pump_sound.clone(),
                at: Some(at.translation),
                volume: None,
                source: Some(owner),
                channel: Some(chimera_sim::weapon::CHAN_ITEM),
            });
        }
        events.write(WeaponEvent {
            owner,
            weapon: active,
            kind: WeaponEventKind::Action(PUMP),
            replay: !first.0,
        });
    }
}

/// After the weapon frame: a shot leaves a pump pending; a dry fire
/// waits its sequence.
pub fn after_frame(mut events: MessageReader<WeaponEvent>, mut shotguns: Query<(&mut Hl2Shotgun, &mut WeaponState)>, clock: Res<SimClock>) {
    for e in events.read() {
        let Ok((mut gun, mut st)) = shotguns.get_mut(e.weapon) else { continue };
        match e.kind {
            WeaponEventKind::Fired { .. } => gun.pump_pending = true,
            WeaponEventKind::DryFire => st.next_primary = clock.now + gun.params.dry as f64,
            _ => {}
        }
    }
}
