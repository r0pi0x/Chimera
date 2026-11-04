//! HL2:DM's spent shells (specs/hl2dm/effects_sounds.md 5, Constants
//! "Shells"): first person only, thrown by Chimera's shells
//! (`map::shells`, `render::shells`) on the view models' numbered brass
//! event (`event_effect`: event 6001, its option the shell type, from
//! attachment 2): the SMG1's fire sequences eject type 0
//! (`models/weapons/shell.mdl`, `Bounce.Shell`), the shotgun's pump type 2
//! at cycle 0.3125 (`models/weapons/shotgun_shell.mdl`,
//! `Bounce.ShotgunShell`); the pistol, .357, AR2 and the rest eject
//! nothing (their view models carry no such event). How they fly
//! (`physics`): f·U(150, 200) along the port plus U(-64, 64), U(-64, 64),
//! U(0, 64) on the world's axes, spin up to 1024 degrees/s on every axis,
//! full gravity (600), half the speed kept per bounce, a life of 1 to 2 s,
//! a bounce sound when landing at 200 units/s or faster, else one hit in
//! four, at full volume from 350. Other players eject nothing (spec 5).
//!
//! Known shortcuts: the fade after the life is CS:S's 2 s (the spec's open
//! question 4); the shell's starting angles are the eye's, not the view
//! model's gun's. Why it's OK for now: both are a frame or two apart from
//! what the real game draws and need its captures to check.

use crate::map::{
    ViewModelEventKind,
    anim::AnimEvent,
    shells::{MapShell, MapShellPhysics},
};
use bevy::prelude::*;
use source_engine::content::material::MaterialLoader;

const UNIT: f32 = 0.0254;

/// The view models' brass event (Source's numbered client event): its
/// option is the shell type.
pub const EVENT_EJECT_BRASS: i32 = 6001;
/// The attachment shells leave from (the spec's attachment 2, 0-based).
pub const PORT: usize = 1;

/// Shell types: the brass event's option, the model, the bounce sound.
pub const SHELLS: &[(&str, &str, &str)] = &[
    ("0", "models/weapons/shell.mdl", "Bounce.Shell"),
    ("2", "models/weapons/shotgun_shell.mdl", "Bounce.ShotgunShell"),
];

/// The model and bounce sound of shell type `key` (the event's option).
pub fn shell(key: &str) -> Option<(&'static str, &'static str)> {
    SHELLS.iter().find(|(k, ..)| *k == key.trim()).map(|(_, m, b)| (*m, *b))
}

/// A view-model event's brass: event 6001 with a known type. HL2's
/// events carry no speed: the speed is `physics`' factor (event speed 1).
pub fn event_effect(e: &AnimEvent) -> Option<ViewModelEventKind> {
    if e.event != EVENT_EJECT_BRASS {
        return None;
    }
    let key = e.options.split_whitespace().next()?;
    shell(key)?;
    Some(ViewModelEventKind::EjectBrass {
        attachment: PORT,
        shell: key.to_string(),
        speed: 1.0,
    })
}

/// How HL2:DM's shells fly (spec 5, view_models.md 8's temp-entity rules
/// with HL2's numbers).
pub fn physics() -> MapShellPhysics {
    MapShellPhysics {
        gravity: 600.0 * UNIT,
        damping: 0.5,
        rest_normal: 0.9,
        rest_steps: 3.0,
        angle_damping: 0.9,
        unit: UNIT,
        speed_factor: (150.0, 200.0),
        up_jitter: 0.0,
        right_jitter: 0.0,
        spin: 1024f32.to_radians(),
        life: 1.0,
        fade: 2.0,
        sound_chance: 0.25,
        sound_full_speed: 350.0 * UNIT,
        sound_sure_speed: Some(200.0 * UNIT),
        world_jitter: (Vec3::new(-64.0, 0.0, -64.0) * UNIT, Vec3::new(64.0, 64.0, 64.0) * UNIT),
        roll_spin: 1024f32.to_radians(),
        life_jitter: 1.0,
        owner_velocity: false,
    }
}

/// The kit's shells read from the install: each type's model, with
/// warnings for those that don't load.
pub fn load(materials: &mut MaterialLoader, warnings: &mut Vec<String>) -> Vec<MapShell> {
    SHELLS
        .iter()
        .filter_map(|(key, path, bounce)| match source_engine::content::props::load_shell(materials, path) {
            Ok(model) => Some(MapShell {
                key: key.to_string(),
                model,
                bounce: Some(bounce.to_string()),
            }),
            Err(e) => {
                warnings.push(e);
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: i32, options: &str) -> AnimEvent {
        AnimEvent { event, options: options.into(), ..Default::default() }
    }

    #[test]
    fn the_brass_event_names_the_type() {
        let Some(ViewModelEventKind::EjectBrass { attachment, shell, speed }) = event_effect(&ev(6001, "0")) else {
            panic!("type 0 ejects");
        };
        assert_eq!((attachment, shell.as_str(), speed), (PORT, "0", 1.0));
        assert!(matches!(event_effect(&ev(6001, "2")), Some(ViewModelEventKind::EjectBrass { shell, .. }) if shell == "2"));
        // The world shotgun's type 3 and other events: nothing.
        assert!(event_effect(&ev(6001, "3")).is_none());
        assert!(event_effect(&ev(5001, "0")).is_none());
    }

    /// Spec test 34: |v_z| 100 sounds one time in four, at 0.5 × 100/350
    /// of the entry's volume (0.5); 200 or more always.
    #[test]
    fn bounce_sounds_34() {
        let p = physics();
        let v = 100.0 * UNIT;
        assert!(p.sounds(v, 0.2) && !p.sounds(v, 0.3));
        assert!(p.sounds(-200.0 * UNIT, 0.99));
        let volume = 0.5 * crate::map::shells::bounce_volume(v, &p);
        assert!((volume - 0.143).abs() < 1e-3, "{volume}");
    }
}
