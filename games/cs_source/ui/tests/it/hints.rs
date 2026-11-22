//! The automatic hints on screen (`ui::hints`): the local player's hint
//! shows as the HUD's hint text with the hints' sound (`Hud.Hint`), in
//! the token's words without the install; another player's doesn't show
//! here.

use bevy::{ecs::message::Messages, prelude::*};

use crate::stack::{
    client::chat::Hint,
    core::{LocalPlayer, Team},
    games::cs_source::{TICK_INTERVAL, ui::hints::HintsHudPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    map::PlaySound,
    movement::placeholder,
    rules::hints::AutoHint,
};

#[test]
fn the_local_players_hint_shows_with_its_sound() {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, HintsHudPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 12.0), placeholder::ID);
    let other = sim.spawn_character(Vec3::new(0.0, 1.0, -20.0), placeholder::ID);
    sim.app.world_mut().entity_mut(me).insert((Team(2), LocalPlayer));
    sim.app.world_mut().entity_mut(other).insert(Team(1));
    sim.ticks(2);
    let mut hints = sim.app.world().resource::<Messages<Hint>>().get_cursor_current();
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    for to in [other, me] {
        sim.app.world_mut().write_message(AutoHint {
            to,
            token: "Cstrike_TitlesTXT_Hint_out_of_ammo".into(),
        });
    }
    sim.ticks(1);
    let w = sim.app.world();
    let shown: Vec<Hint> = hints.read(w.resource::<Messages<Hint>>()).cloned().collect();
    assert_eq!(shown, [Hint("Out of ammo".into())]);
    let played: Vec<String> = sounds
        .read(w.resource::<Messages<PlaySound>>())
        .filter(|s| s.at.is_none())
        .map(|s| s.entry.clone())
        .collect();
    assert_eq!(played, ["Hud.Hint"]);
}
