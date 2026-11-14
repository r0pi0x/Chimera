//! The crossbow (specs/hl2dm/projectiles.md 7), `hl2dm:crossbow`
//! (`Hl2Crossbow`) over the shared frame: clip 1, its rounds bolts
//! (Chimera's `chimera:projectile`: 3500 units/s, 1500 with the shooter's
//! eyes under water, a twentieth of gravity, a ±1 box hitting hitboxes,
//! 100 as a bullet wherever it hits a player (the real server's bolt hits
//! the generic hitgroup: no head bonus, measured; the spec's 300 head
//! shot doesn't happen), glancing off
//! at shallow angles (c < 0.5, faster than 100: 0.75 of its speed, full
//! gravity after), else sticking for 2 s; passing the map's grates and
//! flying on through breakable glass, 100 to the pane: 7.3, 7.4), view
//! punch −2 (`hl2dm:punch`).
//!
//! - **Automatic reload**: on the first tick strictly after the fire
//!   animation's end the reload (1.8333 s, the player busy) starts if
//!   there are bolts in reserve; the bolt is in at its end, where a held
//!   attack fires again. The animation's length is data: 0.1 s (tick 7),
//!   as the real server measures (a shot every 130 ticks held, the load
//!   sound 69 ticks after the shot), not the spec's 0.6667 (tick 45, 168
//!   ticks a shot).
//! - **Zoom**: attack2's press (not its hold) toggles FOV 20 (`weapon::
//!   Zoomed`, the crossbow's own zoom: `OwnZoom`), also while reloading;
//!   firing and reloading keep it; putting the crossbow away unzooms.
//!
//! The bolt's blur, its glow where it sticks and the ragdolls it pins to
//! walls (7.5, with the bolt shown there for 30 s) are `projectile_fx`'s.
//!
//! Known shortcuts: the zoom is a step (the spec's 0.1 s in, 0.2 s out
//! are the client's); the unloaded draw (Open question 5) is the loaded
//! one; `BoltFly` plays at the shooter, not following the bolt (the spec
//! names it with the shot, not its source).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{
    Inventory, Magazine, NetPart, Weapon, WeaponEvent, WeaponEventKind, WeaponState, Zoomed, projectile::FlyerHit,
};

use crate::{
    core::{Damageable, Health, Intent, SimClock},
    map::{GameSound, PlaySound},
};

/// `hl2dm:crossbow`'s parameters (seconds, degrees).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CrossbowParams {
    pub fire_anim: f32,
    pub reload: f32,
    pub zoom_fov: f32,
    /// The bolt meeting what doesn't move (7.4).
    pub hit_world_sound: String,
    /// The bolt stopping in something that takes damage (7.4).
    pub hit_body_sound: String,
}

/// The crossbow's state: when it last fired, attack2 held last tick, the
/// zoom.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Crossbow {
    pub params: CrossbowParams,
    pub fired_at: Option<f64>,
    pub secondary_held: bool,
    pub zoomed: bool,
}

impl NetPart for Hl2Crossbow {
    type Net = (Option<f64>, bool, bool);
    fn net(&self) -> Self::Net {
        (self.fired_at, self.secondary_held, self.zoomed)
    }
    fn set_net(&mut self, (f, s, z): Self::Net) {
        self.fired_at = f;
        self.secondary_held = s;
        self.zoomed = z;
    }
}

/// Before the weapon frame: the zoom's toggle (busy or not) and the
/// automatic reload once the fire animation is over.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    mut owners: Query<(Entity, &Intent, &mut Inventory, &Health), Without<crate::map::interp::NetDrawn>>,
    mut bows: Query<(Entity, &mut Hl2Crossbow, &Weapon, &Magazine, &mut WeaponState)>,
    mut events: MessageWriter<WeaponEvent>,
    clock: Res<SimClock>,
    first: Res<crate::core::FirstTimePredicted>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut bow, w, mag, mut st) in &mut bows {
        let held = w
            .owner
            .and_then(|o| owners.get(o).ok())
            .is_some_and(|(_, _, inv, h)| inv.active == Some(e) && h.current > 0.0);
        if !held {
            if bow.zoomed {
                bow.zoomed = false;
                if let Some(o) = w.owner {
                    commands.entity(o).remove::<Zoomed>();
                }
            }
            continue;
        }
        let Some(owner) = w.owner else { continue };
        let Ok((_, intent, mut inv, _)) = owners.get_mut(owner) else {
            continue;
        };
        if intent.secondary && !bow.secondary_held {
            bow.zoomed = !bow.zoomed;
            if bow.zoomed {
                commands.entity(owner).insert(Zoomed {
                    fov: bow.params.zoom_fov,
                    scope: false,
                    sensitivity: None,
                });
            } else {
                commands.entity(owner).remove::<Zoomed>();
            }
        }
        bow.secondary_held = intent.secondary;
        if let Some(t) = bow.fired_at
            && now > t + bow.params.fire_anim as f64 + 1e-6
        {
            bow.fired_at = None;
            if mag.clip == 0 && mag.reserve > 0 && !st.reloading() {
                let end = now + bow.params.reload as f64;
                st.reload_end = Some(end);
                inv.next_attack = end;
                events.write(WeaponEvent {
                    owner,
                    weapon: e,
                    kind: WeaponEventKind::ReloadStarted,
                    replay: !first.0,
                });
            }
        }
    }
}

/// After the weapon frame: each shot starts the fire animation's clock.
pub fn after_frame(mut events: MessageReader<WeaponEvent>, mut bows: Query<&mut Hl2Crossbow>, clock: Res<SimClock>) {
    for e in events.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. })
            && let Ok(mut bow) = bows.get_mut(e.weapon)
        {
            bow.fired_at = Some(clock.now);
        }
    }
}

/// The bolt's touch sounds (7.4), where flyers are simulated: the world's
/// where it meets what doesn't move, the body's where it stops in
/// something that takes damage.
pub fn bolt_sounds(
    mut hits: MessageReader<FlyerHit>,
    takes: Query<(), Or<(With<Health>, With<Damageable>)>>,
    mut params: Local<Option<CrossbowParams>>,
    mut out: MessageWriter<GameSound>,
) {
    for h in hits.read() {
        if h.weapon != super::CROSSBOW {
            continue;
        }
        let p = params.get_or_insert_with(|| super::launch::hook_params(super::CROSSBOW, super::CROSSBOW_HOOK));
        let entry = if h.world {
            &p.hit_world_sound
        } else if h.gone && (h.character || h.other.is_some_and(|o| takes.contains(o))) {
            &p.hit_body_sound
        } else {
            continue;
        };
        if !entry.is_empty() {
            out.write(GameSound(PlaySound::at(entry.clone(), h.point)));
        }
    }
}
