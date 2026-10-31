//! Skins that are looping sequences of textures (`map::MapSkinFrames`,
//! LithTech's sprite skins): a drawn mesh whose material carries frames
//! (`PropMaterial::skin_frames`) gets its own clock (`SkinClock`) when its
//! material is set, so each drawn copy of a model (a view model, a body,
//! a held weapon) starts at frame 0 when created and runs on its own; every
//! drawn frame the clock advances by the frame's time in whole
//! milliseconds (none while time is paused) and, when the frame changes,
//! the material's base texture is swapped for it (no blending).
//!
//! Each copy needs its own material for its clock to be its own: bodies'
//! and held models' are their own instances (`probe_lit::instance`), and
//! a view model instances the parts that have frames when it is spawned.

use bevy::prelude::*;

use super::prop_material::PropMaterial;
use crate::map::MapSkinFrames;

/// A drawn mesh's clock for its material's skin frames: ms since it was
/// created (wrapped to the sequence), and the frame shown.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkinClock {
    pub t: u32,
    pub shown: usize,
}

/// A mesh given a material with skin frames starts its clock at 0 (set
/// again: restarts).
pub fn start_clock(
    on: On<Insert<MeshMaterial3d<PropMaterial>>>,
    meshes: Query<&MeshMaterial3d<PropMaterial>>,
    materials: Option<Res<Assets<PropMaterial>>>,
    mut commands: Commands,
) {
    let Some(materials) = materials else { return };
    let Ok(handle) = meshes.get(on.entity) else { return };
    let Some(m) = materials.get(&handle.0) else { return };
    if m.skin_frames.len() > 1 {
        commands.entity(on.entity).insert(SkinClock::default());
    } else {
        commands.entity(on.entity).remove::<SkinClock>();
    }
}

/// Advance every clock by this frame's whole milliseconds and show its
/// frame.
pub fn advance(
    time: Res<Time>,
    mut clocks: Query<(&mut SkinClock, &MeshMaterial3d<PropMaterial>)>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
) {
    let Some(mut materials) = materials else { return };
    let delta_ms = (time.delta_secs_f64() * 1000.0) as u32;
    for (mut clock, handle) in &mut clocks {
        // Read first: `get_mut` marks the material changed.
        let Some(m) = materials.get(&handle.0) else { continue };
        let frames = MapSkinFrames {
            frames: (0..m.skin_frames.len()).collect(),
            frame_ms: m.skin_frame_ms,
        };
        let (t, shown) = frames.step(clock.t, delta_ms);
        clock.t = t;
        if shown == clock.shown {
            continue;
        }
        clock.shown = shown;
        if let Some(mut m) = materials.get_mut(&handle.0) {
            m.base = m.skin_frames.get(shown).cloned();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use super::*;

    fn image(images: &mut Assets<Image>) -> Handle<Image> {
        images.add(Image::default())
    }

    /// models_ltb.md R5 on drawn copies: 5 frames of 100 ms; two copies
    /// created 250 ms apart each show their own frame, swapped whole.
    #[test]
    fn each_copy_runs_its_own_clock() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<PropMaterial>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
            .add_observer(start_clock)
            .add_systems(Update, advance);
        let frames: Vec<Handle<Image>> = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (0..5).map(|_| image(&mut images)).collect()
        };
        let material = |app: &mut App| {
            app.world_mut().resource_mut::<Assets<PropMaterial>>().add(PropMaterial {
                base: Some(frames[0].clone()),
                skin_frames: frames.clone(),
                skin_frame_ms: 100,
                ..default()
            })
        };
        let first = material(&mut app);
        let a = app.world_mut().spawn(MeshMaterial3d(first.clone())).id();
        // The first update's time step is 0.
        app.update();
        for _ in 0..5 {
            app.update();
        }
        let second = material(&mut app);
        let b = app.world_mut().spawn(MeshMaterial3d(second.clone())).id();
        app.update();
        let clock = |app: &App, e| *app.world().get::<SkinClock>(e).unwrap();
        assert_eq!(clock(&app, a), SkinClock { t: 300, shown: 3 });
        assert_eq!(clock(&app, b), SkinClock { t: 50, shown: 0 });
        let base = |app: &App, h: &Handle<PropMaterial>| app.world().resource::<Assets<PropMaterial>>().get(h).unwrap().base.clone();
        assert_eq!(base(&app, &first), Some(frames[3].clone()));
        assert_eq!(base(&app, &second), Some(frames[0].clone()));
        // Past the end it loops: 300 + 5 x 50 = 550 -> 50, frame 0.
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(clock(&app, a), SkinClock { t: 50, shown: 0 });
        assert_eq!(base(&app, &first), Some(frames[0].clone()));
        // A material without frames has no clock.
        let plain = app.world_mut().resource_mut::<Assets<PropMaterial>>().add(PropMaterial::default());
        let c = app.world_mut().spawn(MeshMaterial3d(plain)).id();
        app.update();
        assert!(app.world().get::<SkinClock>(c).is_none());
    }
}
