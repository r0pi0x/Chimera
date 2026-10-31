//! The respawn countdown (`client::respawn_countdown`): hidden while the
//! local player lives or nothing respawns it on a timer (plain CS:S and
//! Combat Arms), "Respawning in N" while dead with a `core::RespawnDue`,
//! counting down in whole seconds, "Respawning" once due.

use bevy::prelude::*;
use chimera_client::client::respawn_countdown::{RespawnCountdownPlugin, RespawnCountdownRoot, RespawnCountdownText, text};
use chimera_core::core::{Dead, LocalPlayer, RespawnAt, RespawnDue, SimClock};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

fn shown(sim: &mut Sim) -> Option<String> {
    let w = sim.app.world_mut();
    let v = *w.query_filtered::<&Visibility, With<RespawnCountdownRoot>>().single(w).unwrap();
    let t = w.query_filtered::<&Text, With<RespawnCountdownText>>().single(w).unwrap().0.clone();
    (v != Visibility::Hidden).then_some(t)
}

#[test]
fn the_text_counts_whole_seconds_up() {
    assert_eq!(text(2.0), "Respawning in 2");
    assert_eq!(text(1.3), "Respawning in 2");
    assert_eq!(text(0.2), "Respawning in 1");
    assert_eq!(text(0.0), "Respawning");
    assert_eq!(text(-0.1), "Respawning");
}

#[test]
fn shown_only_while_dead_with_a_timed_respawn() {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, RespawnCountdownPlugin));
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.ticks(2);
    assert_eq!(shown(&mut sim), None, "alive");
    // Dead, no timed respawn (plain CS:S, Combat Arms): nothing.
    let now = sim.app.world().resource::<SimClock>().now;
    sim.app.world_mut().entity_mut(me).insert(Dead { since: now });
    sim.ticks(2);
    assert_eq!(shown(&mut sim), None, "no timed respawn");
    // A minigame's 2 s.
    sim.app.world_mut().insert_resource(RespawnDue(Some(RespawnAt { at: now + 2.0, delay: 2.0 })));
    sim.ticks(2);
    assert_eq!(shown(&mut sim).as_deref(), Some("Respawning in 2"));
    sim.seconds(1.2);
    assert_eq!(shown(&mut sim).as_deref(), Some("Respawning in 1"));
    sim.seconds(1.0);
    assert_eq!(shown(&mut sim).as_deref(), Some("Respawning"), "due, the respawn on its way");
    sim.app.world_mut().entity_mut(me).remove::<Dead>();
    sim.ticks(2);
    assert_eq!(shown(&mut sim), None, "alive again");
}

/// The countdown is sized for a 720-line screen and grows with the
/// window (it was a fixed 28 px, small on a large screen): 1.5 times at
/// 1080 lines, twice at 1440, and again when the window changes.
#[test]
fn the_countdown_scales_with_the_windows_height() {
    use bevy::window::{PrimaryWindow, WindowResolution};
    use chimera_client::client::respawn_countdown::SIZE;
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, RespawnCountdownPlugin));
    let window = sim
        .app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::from((1920u32, 1080u32)), ..default() }, PrimaryWindow))
        .id();
    sim.ticks(2);
    let size = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query_filtered::<&TextFont, With<RespawnCountdownText>>().single(w).unwrap().font_size
    };
    assert_eq!(size(&mut sim), FontSize::Px(SIZE * 1.5));
    sim.app.world_mut().get_mut::<Window>(window).unwrap().resolution = WindowResolution::from((2560u32, 1440u32));
    sim.ticks(2);
    assert_eq!(size(&mut sim), FontSize::Px(SIZE * 2.0));
}
