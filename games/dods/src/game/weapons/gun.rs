//! A DoD:S gun's own rules over the shared weapon frame (`dods:gun`,
//! `DodsGun`; specs/dods/weapons.md):
//!
//! - **Spread** (weapons.md, "Spread"): the script's `Accuracy` as the
//!   cone, `SecondaryAccuracy` while an alt mode is on (`AltModes` past its
//!   first mode: the sights, the scope, the BAR's and StG44's semi-auto)
//!   or a machine gun is deployed (`stance::DodsStance::deployed`), plus
//!   the whole `AccuracyMovePenalty` while its holder's horizontal speed
//!   after the tick's movement is 45 units/s or more (`MOVING`; 45.0
//!   itself counts, spread.md); no stance bonus, no growth over a burst
//!   (`before_frame`). Each bullet is drawn inside it as DoD:S's pattern
//!   (specs/dods/spread.md: `SpreadShape::TemplateDisc`, Source's
//!   generator seeded with the shot seed, the template's draw cut to the
//!   unit disc), so the same command seed hits the same spot.
//! - **Sights block the trigger**: raising or lowering the sights or
//!   scope (the data's `[alt]` scope, `step` 0.49 s) blocks the next shot
//!   for that time (measured on the Garand, weapons.md "Alt fire"): the
//!   hook makes the step block the primary (`AltModes::blocks_primary`).
//! - **Reload only when empty** (`reload_when_empty`, the Garand's:
//!   weapons.md "Fire modes"): the reload key does nothing while rounds
//!   are left (`WeaponState::reload_held`); a machine gun (`dods:mg`)
//!   reloads only deployed (measured).
//! - **Partial reloads** (weapons.md, "Draw and reload times"): the empty
//!   reload's time, but `partial_reload` (the Colt's 1.56 s, the C96's
//!   2.22 s) with rounds left; only the missing rounds leave the reserve
//!   (the shared frame's).
//! - **The sights' and scopes' field of view** (the data's `[alt]` FOV:
//!   55 the sights, 20 the scopes, measured): the view narrows
//!   `ZOOM_DELAY` (0.315 s, 21 ticks) after the press and widens at once
//!   (`sights`, the gun's own zoom: `OwnZoom`); with it on, its holder
//!   moves at most 42 units/s (`stance::SIGHTED_CAP`).
//! - **Fire select** (the BAR's and StG44's alt fire, the data's burst of
//!   one): semi-automatic, the next shot 0.3 s on, `SecondaryAccuracy` the
//!   spread (0.02, 0.12 moving: measured).
//! - **The SMGs' stock strike** (the Thompson's and MP40's alt fire): a
//!   melee swing of the data's (`[melee.secondary]`, the gun firing on the
//!   attack: `Melee::gun_primary`), the knife's timing and reach, 60, no
//!   backstab (damage.md, "Melee").
//!
//! Known shortcuts (docs/tech-debt.md):
//! - `Recoil` is kept in the parameters but gives no view kick: its shape
//!   and recovery are unmeasured (weapons.md, open question 8).
//! - The field of view changes in one step (measured: over 0.1 s); the
//!   view model doesn't move to its iron-sight offset.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{AltModes, Hitscan, Inventory, Magazine, OwnZoom, SpreadShape, WeaponState, Zoom, Zoomed};

use crate::core::Velocity;

/// Horizontal speed from which a holder's shot takes the move penalty,
/// m/s (weapons.md, "Spread"; spread.md: 45.0 units/s counts, measured).
pub const MOVING: f32 = 45.0 * 0.0254;

/// Whether a holder moving at `velocity` (m/s) takes the move penalty:
/// its horizontal speed `MOVING` or more (a hair under it for float
/// rounding: a steady 45.0 measured as moving).
pub fn moving(velocity: Vec3) -> bool {
    velocity.with_y(0.0).length() >= MOVING - 1e-5
}

/// From the sights' or scope's press to the narrower view, seconds
/// (measured: 21 ticks).
pub const ZOOM_DELAY: f64 = 0.315;

/// A sighted gun's mode and since when (game seconds), for `sights`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DodsSights {
    pub mode: u8,
    pub since: f64,
}

/// `dods:gun`'s parameters: the script's numbers (`script_numbers`).
#[derive(Component, Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DodsGun {
    /// The script it reads (`weapon_garand`).
    pub script: String,
    /// `Accuracy`: the cone standing still with the sights down.
    pub accuracy: f32,
    /// `SecondaryAccuracy`: the cone with the sights up (none: the gun
    /// has none).
    #[serde(default)]
    pub alt_accuracy: Option<f32>,
    /// `AccuracyMovePenalty`: added while moving.
    #[serde(default)]
    pub move_penalty: f32,
    /// `Recoil`, degrees (no kick yet: module doc).
    #[serde(default)]
    pub recoil: f32,
    /// The reload key does nothing while the clip holds rounds.
    #[serde(default)]
    pub reload_when_empty: bool,
    /// A reload with rounds left takes this long, seconds (none: as an
    /// empty one, the data's `[magazine] reload`).
    #[serde(default)]
    pub partial_reload: Option<f32>,
}

/// A gun's reload times, empty and with rounds left (`partial_reload`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ReloadTimes {
    pub empty: f32,
    pub partial: f32,
}

impl DodsGun {
    /// The cone for a shot: sighted or not, moving or not.
    pub fn cone(&self, sighted: bool, moving: bool) -> f32 {
        let base = match (sighted, self.alt_accuracy) {
            (true, Some(a)) => a,
            _ => self.accuracy,
        };
        base + if moving { self.move_penalty } else { 0.0 }
    }
}

/// On a gun's entity as the hook puts it: its sights' step blocks the
/// trigger.
pub fn insert(e: &mut EntityWorldMut, p: &DodsGun) {
    if let Some(mut m) = e.get_mut::<AltModes>() {
        m.blocks_primary = true;
    }
    if e.contains::<Zoom>() {
        e.insert((OwnZoom, DodsSights::default()));
    }
    if let (Some(partial), Some(empty)) = (p.partial_reload, e.get::<Magazine>().map(|m| m.reload_time)) {
        e.insert(ReloadTimes { empty, partial });
    }
    e.insert(p.clone());
}

/// Before the weapon frame: the cone for this tick's shot and the
/// Garand's reload hold.
#[allow(clippy::type_complexity)]
pub fn before_frame(
    owners: Query<(&Inventory, Option<&Velocity>, Option<&crate::game::stance::DodsStance>), Without<crate::map::interp::NetDrawn>>,
    mut guns: Query<(&DodsGun, &mut Hitscan, &mut WeaponState, Option<&AltModes>, Option<&mut Magazine>, Option<&ReloadTimes>, Has<super::mg::DodsMg>)>,
) {
    for (inv, velocity, stance) in &owners {
        let Some(Ok((gun, mut scan, mut st, alt, mut mag, times, mg))) = inv.active.map(|a| guns.get_mut(a)) else { continue };
        if let (Some(t), Some(m)) = (times, mag.as_mut()) {
            let want = if m.clip > 0 { t.partial } else { t.empty };
            if m.reload_time != want {
                m.reload_time = want;
            }
        }
        let mag = mag.as_deref();
        let moving = velocity.is_some_and(|v| moving(v.0));
        let sighted = alt.is_some_and(|a| a.current > 0) || stance.is_some_and(|s| s.deployed);
        let cone = SpreadShape::TemplateDisc(gun.cone(sighted, moving));
        if scan.spread != cone {
            scan.spread = cone;
        }
        if gun.reload_when_empty || mg {
            let held = (gun.reload_when_empty && mag.is_some_and(|m| m.clip > 0)) || (mg && !stance.is_some_and(|s| s.deployed));
            if st.reload_held != held {
                st.reload_held = held;
            }
        }
    }
}

/// After the weapon frame: the view of a sighted gun's holder, narrowed
/// `ZOOM_DELAY` after the sights went up, wide at once when they go down.
#[allow(clippy::type_complexity)]
pub fn sights(
    owners: Query<(Entity, &Inventory, Option<&Zoomed>), Without<crate::map::interp::NetDrawn>>,
    mut guns: Query<(&Zoom, &AltModes, &mut DodsSights)>,
    clock: Res<crate::core::SimClock>,
    mut commands: Commands,
) {
    for (owner, inv, zoomed) in &owners {
        let Some(Ok((zoom, alt, mut s))) = inv.active.map(|a| guns.get_mut(a)) else { continue };
        if s.mode != alt.current {
            *s = DodsSights { mode: alt.current, since: clock.now };
        }
        let want = (s.mode > 0 && clock.now + 1e-6 >= s.since + ZOOM_DELAY)
            .then(|| zoom.fov.get(s.mode as usize - 1).map(|f| Zoomed { fov: *f, scope: zoom.scope, sensitivity: None }))
            .flatten();
        if zoomed != want.as_ref() {
            match want {
                Some(z) => commands.entity(owner).insert(z),
                None => commands.entity(owner).remove::<Zoomed>(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cone_is_the_scripts_by_sights_and_movement() {
        let g = DodsGun {
            script: "weapon_garand".into(),
            accuracy: 0.014,
            alt_accuracy: Some(0.0),
            move_penalty: 0.1,
            recoil: 4.0,
            reload_when_empty: true,
            partial_reload: None,
        };
        assert_eq!(g.cone(false, false), 0.014);
        assert_eq!(g.cone(true, false), 0.0);
        assert!((g.cone(false, true) - 0.114).abs() < 1e-6);
        let smg = DodsGun { alt_accuracy: None, ..g };
        assert_eq!(smg.cone(true, false), 0.014, "no sights: the base cone");
    }
}
