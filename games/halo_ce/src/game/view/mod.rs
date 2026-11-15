//! Halo's guns seen in hand (docs/plans/active/halo-ce.md, M2:
//! "first-person models and animations"; specs/halo_ce/animations.md
//! "First person", weapons.md 2, 11, 12, 16, models.md): the first-person
//! view model of each gun with the Spartan's arms, played by the view's
//! own state machine in step with the weapons' 30 Hz ticks, and the gun in
//! the Spartan's hand in third person with its stance.
//!
//! - `load`: the view and held models from a map file (named by the weapon
//!   documents' `[models]`), on Halo's maps and, from the donor map, on
//!   any other (`load::GUEST`); the muzzle and ejection attachments "for
//!   effects" are named there.
//! - `machine`: the view's states, overlays, smoothed inputs and blends,
//!   pure.
//!
//! **Driving** (`drive`, `FixedLast`, after the weapons' tick; simulation
//! side, so it runs headless and is tested there): a holder of a Halo gun
//! shows its view model (`ViewAnimator::show`: the engine's generic
//! driver, `weapon::view_anim`, keeps the key and the hiding; Halo
//! registers no sequences or rules with it, so it plays nothing on them),
//! and each 30 Hz tick (`weapons::halo_ticks`: one per engine tick at
//! Halo's own rate, paced under another game's) its `HaloViewModel` runs
//! one tick of `machine::HaloView` with the weapon's events of that engine
//! tick as cues (`cue`: `Deployed` readies, `fire`/`fire_last` fire,
//! `misfire`, `melee`, `ReloadStarted` reloads (the rounds loaded pick
//! empty or full, the shells needed count the shotgun's loop),
//! `ShellInserting` the next shell, `overheated`, `overheat_cleared`; the
//! weapon's `overheat_exit` its exit) and the holder's throttle and look.
//! The pose is laid out by the game (`Animator::fixed`, sequence 0 named
//! as playing) and held between ticks (animations.md "Per-tick order": no
//! interpolation). The engine's `ViewAnimator` and its sequence driver
//! assume Source's (sequences played in seconds, one at a time); Halo's
//! view stacks overlays and smoothed inputs on a state animation counted
//! in ticks, so it is Halo's own small driver beside it, not rules for it.
//!
//! **What each gun plays** (the first-person graphs' slots): ready on a
//! draw (29, 35, 29, 23 ticks: the weapons' draw), idle and posing; the
//! pistol (15), plasma rifle (22) and shotgun (26) their fire-1 on a shot,
//! the assault rifle none (its push back moves the overlays' frame 8);
//! reload-empty or reload-full (assault rifle 87 both; pistol 67 / 65);
//! the shotgun its enter (15), a reload-empty per further shell (12), then
//! exit-empty (42) or exit-full (24); melee (37, 45, 36, 36); the plasma
//! rifle's misfire (18), overheating (58), overheated (held), its exit
//! (35); the moving overlay while walking. Put-away is never seen: a
//! switch readies the new gun in the same tick (weapons.md 2). The
//! arsenal's other five: the sniper rifle's fire-1 (19), reloads (94 / 83);
//! the rocket launcher's fire-1 (28), reloads (125 / 111); the needler's
//! fire-1 (8), reload (70) and its needles as the ammunition overlay (the
//! rounds loaded, refilling late in the reload); the plasma pistol's fire-1
//! (13) on a bolt, charged (`charged`: its `overcharged` slot, 20, with the
//! overcharged jitter while held) and fire-2 (24) on the charged bolt
//! (`fire_charged`), its overheat states (34, 25, 9, exit 24) and misfire
//! (19); the flamethrower no fire-1 (the push back only), reload (104).
//! Every gun's throw-grenade (37 to 43 frames) on `throw_grenade`, its
//! throw-overheated on an overheated plasma weapon.
//!
//! **Zoom** (weapons.md 12, animations.md "Root at the camera"): the view
//! is hidden while zoomed (`ViewModelHidden` while the holder has
//! `Zoomed` and a Halo gun), at every level (the sniper rifle's 2x and
//! 8x, the pistol's and rocket launcher's 2x); the camera's zoom is the
//! weapons'.
//!
//! **Placement**: no offset (the models are authored relative to the
//! camera, node 0 at its frame) and the camera's field of view (`settings`,
//! `fov`). **Field of view** (weapons.md 12, objects.md 0x1A0): the unit's
//! 70 degrees is a nominal angle a; the camera's vertical field of view v
//! has tan(v / 2) = 0.75 x 0.85 x tan(a / 2) (48.11 degrees), kept on
//! every window shape (the horizontal is v widened by the aspect: 61.52
//! degrees at 4:3, 76.87 at 16:9). In the engine's terms (degrees
//! horizontal at 4:3, `map::view_model::vertical_fov`) that is
//! 2 atan(0.85 tan(a / 2)): `camera_fov` of a nominal angle, `FOV` of the
//! unit's; the weapons' zoom (`Zoomed`) takes `camera_fov` of the nominal
//! angle divided by the magnification, so the world camera, the view
//! model's camera and the effects' re-projection between them
//! (`fx::place`, which reads both cameras) all use the real angles.
//!
//! **Third person** (`stance`): a holder of a Halo gun takes its label
//! (`body::WeaponLabel` from the document's `label`, the weapon tag's) and
//! holds its held model (`map::Held`).
//!
//! Known shortcuts: see `machine` and `load`; the view's sounds (each
//! state animation's sound frame) are the sound step's.

pub mod load;
pub mod machine;

use bevy::prelude::*;
use chimera_core::later::Later;
use chimera_sim::weapon::{
    Inventory, Magazine, ViewModelHidden, Weapon, WeaponEvent, WeaponEventKind, Zoomed, view_anim::ViewAnimPlugin,
};

use self::machine::{Cue, HaloView, Tick};
use super::{
    body::{WeaponLabel, inputs},
    weapons::{HaloGun, HaloWeapon, halo_ticks, tick::MagazineMode},
};
use crate::{
    core::{Health, Intent, MovementState, SimClock, Velocity},
    map::{GameFov, Held, ViewAnimator, ViewModelSettings, ViewModels},
};

/// The unit's nominal camera field of view, degrees (objects.md 0x1A0: 70).
pub const NOMINAL_FOV: f32 = 70.0;

/// The renderer's fixed narrowing of the nominal angle's half tangent
/// (weapons.md 12).
pub const NARROWING: f32 = 0.85;

/// The camera's field of view for a nominal angle (degrees), in the
/// engine's terms: degrees horizontal at 4:3, 2 atan(0.85 tan(a / 2))
/// (module doc; its vertical is `map::view_model::vertical_fov` of it).
pub fn camera_fov(nominal: f32) -> f32 {
    2.0 * (NARROWING * (nominal.to_radians() / 2.0).tan()).atan().to_degrees()
}

/// The camera's field of view (module doc): `camera_fov(NOMINAL_FOV)`,
/// 61.52 degrees horizontal at 4:3 (48.11 vertical): the world camera's
/// in Halo's app and the view model's.
pub const FOV: f32 = 61.520_22;

/// The view-model settings Halo's own app starts with: the models at the
/// camera's field of view, whatever the player's (`SettingsFollowPlayerFov`).
pub fn settings() -> ViewModelSettings {
    ViewModelSettings {
        fov: FOV,
        default_fov: FOV,
        right_hand: 1,
        draw: 1,
    }
}

/// Its view models' field of view in an app of several games.
pub fn fov() -> GameFov {
    GameFov {
        game: super::GAME,
        fov: FOV,
        default_fov: FOV,
        follows_player: true,
    }
}

/// The map's globals' first-person idle numbers (globals.md; the view's
/// posing timer, `machine::set_idle`); the fallback where the map has none.
pub fn idle_of(file: &blam_formats::cache::MapFile) -> machine::IdleTiming {
    let read = || {
        let t = file.find(blam_formats::cache::class::GLOBALS, "globals\\globals")?;
        let p = blam_formats::globals::Globals::read(file, t).ok()?.player;
        Some(machine::IdleTiming {
            time: (p.first_person_idle_time.lower, p.first_person_idle_time.upper),
            skip: p.first_person_idle_skip,
        })
    };
    read().unwrap_or(machine::IdleTiming::FALLBACK)
}

/// Each gun's label (its document's), by id.
pub fn labels() -> Vec<(&'static str, String)> {
    super::weapons::guns()
        .into_iter()
        .filter_map(|(id, g)| Some((super::weapons::GUNS.into_iter().find(|g| *g == id)?, g.label)))
        .collect()
}

/// A weapon event as the view's cue (module doc); `magazine`: the rounds
/// loaded, the magazine's size and the reserve, at the event.
pub fn cue(kind: &WeaponEventKind, magazine: Option<(u32, u32, u32)>) -> Option<Cue> {
    Some(match kind {
        WeaponEventKind::Deployed => Cue::Ready,
        WeaponEventKind::Action("fire" | "fire_last") => Cue::Fire,
        WeaponEventKind::Action("misfire") => Cue::Misfire,
        WeaponEventKind::Action("melee") => Cue::Melee,
        WeaponEventKind::Action("overheated") => Cue::Overheat,
        WeaponEventKind::Action("overheat_cleared") => Cue::OverheatCleared,
        WeaponEventKind::Action("fire_charged") => Cue::FireSecondary,
        WeaponEventKind::Action("charged") => Cue::Charged,
        WeaponEventKind::Action("throw_grenade") => Cue::Throw,
        WeaponEventKind::ReloadStarted => {
            let (loaded, size, reserve) = magazine.unwrap_or_default();
            Cue::Reload {
                loaded,
                needed: size.saturating_sub(loaded).min(reserve),
            }
        }
        WeaponEventKind::ShellInserting => Cue::Shell,
        _ => return None,
    })
}

/// A holder's first-person view of a Halo gun: the machine, the look at
/// the last tick, and whether it hid the view model; on every character
/// from its spawn, its key empty while it holds no Halo gun.
#[derive(Component, Clone, Debug, Default)]
pub struct HaloViewModel {
    pub key: String,
    pub view: HaloView,
    pub look: Option<Vec2>,
    pub hid: bool,
}

/// Holders of Halo guns, one tick (module doc).
#[allow(clippy::type_complexity)]
pub fn drive(
    clock: Res<SimClock>,
    models: Option<Res<ViewModels>>,
    mut holders: Query<(
        Entity,
        &Inventory,
        &Intent,
        (
            Option<&Health>,
            Option<&Velocity>,
            Option<&MovementState>,
            Option<&Zoomed>,
        ),
        Option<&mut ViewAnimator>,
        Option<&mut HaloViewModel>,
    )>,
    guns: Query<(&Weapon, &HaloGun, Option<&Magazine>, &HaloWeapon)>,
    mut events: MessageReader<WeaponEvent>,
    mut commands: Commands,
) {
    let events: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    let n = halo_ticks(&clock);
    for (e, inv, intent, (health, velocity, state, zoomed), view, own) in &mut holders {
        let alive = health.is_none_or(|h| h.current > 0.0);
        let gun = inv.active.filter(|_| alive).and_then(|w| guns.get(w).ok());
        let Some((weapon, halo, magazine, numbers)) = gun else {
            if let Some(mut own) = own.filter(|o| !o.key.is_empty()) {
                if own.hid {
                    commands.entity(e).remove::<ViewModelHidden>();
                }
                *own = HaloViewModel::default();
            }
            continue;
        };
        let Some(mut view) = view else { continue };
        view.show(Some(weapon.id), models.as_deref());
        let Some(set) = view
            .animator
            .as_ref()
            .map(|a| a.set.clone())
            .filter(|s| s.sequences.len() > 1)
        else {
            continue;
        };
        let Some(mut own) = own else { continue };
        let fresh = own.key != weapon.id;
        if fresh {
            let hid = own.hid;
            *own = HaloViewModel {
                key: weapon.id.to_string(),
                view: HaloView::new(e.to_bits() as u32 ^ 0x6a09_e667),
                look: None,
                hid,
            };
            own.view.cue(&set, Cue::Ready, halo.overheated);
        }
        let mag = magazine.map(|m| (m.clip, m.size, m.reserve));
        let mut cues: Vec<Cue> = events
            .iter()
            .filter(|ev| ev.owner == e)
            .filter_map(|ev| cue(&ev.kind, mag))
            .collect();
        if fresh {
            cues.retain(|c| *c != Cue::Ready);
        }
        let look = Vec2::new(intent.yaw, intent.pitch);
        // The ammunition overlay's frame (the needler's needles).
        let ammunition = numbers.0.magazine.filter(|m| m.rounds_loaded_maximum > 0).map(|m| {
            let reloading = halo.magazine == MagazineMode::Reloading;
            let length = numbers.0.animations.reload_empty.unwrap_or(0);
            machine::ammunition_frame(
                i32::from(halo.loaded),
                reloading.then_some((length, halo.magazine_timer)),
                i32::from(halo.loaded) + i32::from(halo.reserve),
                i32::from(m.rounds_loaded_maximum),
            )
        });
        let mut pose = None;
        // A fresh view shows the draw's frame 0 on this tick.
        for k in 0..if fresh { n.saturating_sub(1) } else { n } {
            let turn = own.look.map_or(Vec2::ZERO, |l| {
                let d = look - l;
                Vec2::new(
                    (d.x + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI,
                    d.y,
                )
            });
            own.look = Some(look);
            let i = match (velocity, state) {
                (Some(v), Some(s)) => inputs(intent, v.0, s),
                _ => Default::default(),
            };
            let t = Tick {
                cues: if k == 0 { std::mem::take(&mut cues) } else { Vec::new() },
                throttle: i.throttle,
                airborne: !i.grounded,
                turn,
                zoomed: zoomed.is_some(),
                overheated: halo.overheated,
                overheat_exit: halo.overheat_exit,
                overcharge: halo.overcharged,
                ammunition,
            };
            pose = Some(own.view.tick(&set, &t));
        }
        if fresh && pose.is_none() {
            pose = Some(own.view.pose(&set));
        }
        if let (Some(pose), Some(a)) = (pose, view.animator.as_mut()) {
            a.fixed = Some(Later::ready(pose));
            a.main = Some(0);
        }
        // Hidden while zoomed (animations.md "Root at the camera").
        let hide = zoomed.is_some();
        if hide != own.hid {
            own.hid = hide;
            if hide {
                commands.entity(e).insert(ViewModelHidden);
            } else {
                commands.entity(e).remove::<ViewModelHidden>();
            }
        }
    }
}

/// Holders of Halo guns take its stance and hold it (module doc).
pub fn stance(
    holders: Query<(Entity, &Inventory, Option<&WeaponLabel>, Option<&Held>)>,
    guns: Query<(&Weapon, &HaloWeapon)>,
    mut commands: Commands,
) {
    for (e, inv, label, held) in &holders {
        match inv.active.and_then(|w| guns.get(w).ok()) {
            Some((weapon, halo)) => {
                if !halo.0.label.is_empty() && label.is_none_or(|l| l.0 != halo.0.label) {
                    commands.entity(e).insert(WeaponLabel(halo.0.label.clone()));
                }
                if held.is_none_or(|h| h.0.as_deref() != Some(weapon.id)) {
                    commands.entity(e).insert(Held(Some(weapon.id.to_string())));
                }
            }
            None if inv.active.is_none()
                && held.is_some_and(|h| h.0.as_deref().is_some_and(|k| k.starts_with("halo_ce:"))) =>
            {
                commands.entity(e).insert(Held(None));
            }
            None => {}
        }
    }
}

/// Halo's first-person view and held guns (module doc); the engine's view
/// driver (which shows and hides the view model) where the app has none.
pub struct HaloViewPlugin;

impl Plugin for HaloViewPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        if !app.is_plugin_added::<ViewAnimPlugin>() {
            app.add_plugins(ViewAnimPlugin);
        }
        // What a character holds, its view and its label are on it from the spawn (a part that
        // came and went would make archetypes: docs/DESIGN.md, "ECS style").
        app.on_every_character::<Held>()
            .on_every_character::<HaloViewModel>()
            .on_every_character::<WeaponLabel>();
        app.add_systems(
            FixedLast,
            (stance.before(super::body::animate), drive.before(crate::map::SimPose)),
        );
    }
}
