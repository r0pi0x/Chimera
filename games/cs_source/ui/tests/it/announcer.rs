//! Plain CS:S has no announcer (specs/combat_arms/kill_streaks.md is
//! Combat Arms'; `chimera_client::client::announcer`): no game of the app
//! contributes a pack, so a double kill shows no banner and plays no
//! voice, and the server announces nothing.

use bevy::prelude::*;
use chimera_core::announcer::{ActiveAnnouncer, AnnouncerPacks, KillAnnounced};

use crate::stack::{
    client::announcer::{AnnouncerPlugin, AnnouncerView},
    core::{Damage, Hitgroup, LocalPlayer, Team},
    harness::{CsSim, Sim},
    movement::placeholder,
};

#[derive(Resource, Default)]
struct Announced(usize);

fn count(mut kills: MessageReader<KillAnnounced>, mut n: ResMut<Announced>) {
    n.0 += kills.read().count();
}

#[test]
fn a_double_kill_announces_nothing() {
    let mut sim = Sim::new(CsSim, AnnouncerPlugin);
    sim.app.init_resource::<Announced>().add_systems(Last, count);
    let add = |sim: &mut Sim, x: f32, team: u8| {
        let e = sim.spawn_character(Vec3::new(x, 1.0, 0.0), placeholder::ID);
        sim.app.world_mut().entity_mut(e).insert(Team(team));
        e
    };
    let me = add(&mut sim, 0.0, 2);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let (a, b) = (add(&mut sim, 3.0, 3), add(&mut sim, -3.0, 3));
    sim.seconds(0.5);
    assert!(sim.app.world().resource::<AnnouncerPacks>().0.is_empty(), "CS:S has no pack");
    assert!(sim.app.world().resource::<ActiveAnnouncer>().0.is_none());
    for victim in [a, b] {
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target: victim,
            attacker: Some(me),
            amount: 10.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Head,
            kind: Default::default(),
            weapon: None,
        });
        sim.ticks(2);
    }
    assert!(sim.app.world().get::<crate::stack::core::Dead>(b).is_some() || sim.app.world().get::<crate::stack::core::Health>(b).is_some_and(|h| h.current <= 0.0), "both killed");
    assert_eq!(sim.app.world().resource::<Announced>().0, 0);
    let view = sim.app.world().resource::<AnnouncerView>();
    assert!(view.player.shown.is_none() && view.voices.is_empty());
}
