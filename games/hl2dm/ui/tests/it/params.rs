//! The plugins' systems initialize: every system's parameters are checked
//! for conflicting access (Bevy's B0002, e.g. a `Res` and a `ResMut` of the
//! same assets in one system) when the schedule is built, so a conflict
//! fails here, in the crate that owns the system, not only in an app that
//! adds it (party_kit's HUD tests). Building runs no system: no resources needed.

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

#[test]
fn the_hud_plugins_systems_have_no_conflicting_params() {
    let mut app = App::new();
    app.add_plugins(hl2dm_ui::Hl2dmHudPlugin);
    for label in [Startup.intern(), Update.intern()] {
        app.world_mut().schedule_scope(label, |world, schedule| {
            schedule.initialize(world).expect("the schedule builds");
        });
    }
}
