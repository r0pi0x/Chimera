//! The party's gun game for the tests: the party's simulation on the
//! greybox with the gun game started, scripted soldiers, aim and kills.
//! Every test binary includes it, so every binary names the games the
//! default party plays (Combat Arms the base, the original gun game's
//! CS:S and Combat Arms guns): their changes select every binary
//! (checks/src/affected.rs, "tests by game").

#![allow(dead_code, unused_imports)]

use avian3d::prelude::Position;
use bevy::prelude::*;
use chimera_core::{
    core::{Dead, God, Score, Team},
    gameplay::Gameplay,
};
use chimera_sim::{
    minigame::{self, MINIGAME, MinigameEvent, ladder::Rung},
    mode::{Match, Phase},
    testkit,
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::movement, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// The default party's games (see the module doc).
use {combat_arms as _, cs_source as _, originals as _};

/// The party's simulation on the greybox, the gun game started (its
/// rung extras off: `plain_rungs`).
pub(crate) fn party() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim
}

/// The host's setting "Knife finale: off" for this world: the tests of
/// the guns' ladder (ten rungs) predate the melee finale, which is on by
/// default (`the_gun_game_ends_on_a_melee_rung_by_default` and the full
/// game test play it).
pub(crate) fn no_finale(world: &mut World) {
    world
        .get_resource_or_init::<chimera_sim::minigame::overrides::MinigameOverrides>()
        .keys
        .insert("rules.knife_finale".into(), "false".into());
}

/// The ladders this world makes from now on draw no rung extras (no
/// game's hook: `ladder::RungExtrasHooks` gone), as before them: for the
/// tests of everything else (the kit's knife, one's own look and the
/// guns' own parts on every rung); rung_extras.rs tests them.
pub(crate) fn plain_rungs(world: &mut World) {
    world.remove_resource::<chimera_sim::minigame::ladder::RungExtrasHooks>();
}

/// A Combat Arms soldier, not yet in the game (the minigame takes it in).
/// The greybox's open middle, where `line_up` stands players (feet just
/// above the floor).
pub(crate) const OPEN: Vec3 = Vec3::new(0.0, 1.0, 0.0);

pub(crate) fn soldier(sim: &mut Sim) -> Entity {
    sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID)
}

pub(crate) fn place(sim: &mut Sim, e: Entity, at: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    w.get_mut::<Position>(e).unwrap().0 = at;
}

pub(crate) fn held(sim: &Sim, e: Entity) -> Option<&'static str> {
    testkit::active_id(sim.app.world(), e)
}

pub(crate) fn rung(sim: &Sim, e: Entity) -> usize {
    sim.app.world().get::<Rung>(e).map_or(0, |r| r.0)
}

pub(crate) fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<Match>().phase
}

pub(crate) fn alive(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<Dead>(e).is_none()
}

/// `shooter` aims at `target`'s centre and works the trigger (pressed
/// every other tick, for the semi-automatics) until `target` dies or
/// `secs` pass; whether it died.
pub(crate) fn shoot_until_dead(sim: &mut Sim, shooter: Entity, target: Entity, secs: f64) -> bool {
    let ticks = (secs * sim.tick_hz()) as u64;
    for t in 0..ticks {
        let at = sim.position(target);
        testkit::aim_at(sim, shooter, at);
        // Held against the recoil (the view punch the shots climb by), as
        // a player pulls down: a long burst otherwise climbs over the head.
        let punch = sim.app.world().get::<chimera_sim::weapon::ViewPunch>(shooter).map_or(Vec3::ZERO, |p| p.0);
        {
            let mut i = sim.intent(shooter);
            i.pitch -= punch.x;
            i.yaw -= punch.y;
            i.fire = t % 2 == 0;
        }
        sim.ticks(1);
        if !alive(sim, target) {
            sim.intent(shooter).fire = false;
            return true;
        }
    }
    sim.intent(shooter).fire = false;
    false
}

/// `target` back in play, unprotected, `d` m in front of `shooter` (both
/// standing still, on the greybox's open middle: a spawn spot may be near
/// a ledge), the shooter's weapon drawn.
pub(crate) fn line_up(sim: &mut Sim, shooter: Entity, target: Entity, d: f32) {
    for _ in 0..(6.0 * sim.tick_hz()) as u32 {
        if alive(sim, target) && sim.app.world().get::<God>(target).is_none() {
            break;
        }
        sim.ticks(1);
    }
    assert!(alive(sim, target), "respawned");
    let from = OPEN;
    place(sim, shooter, from);
    place(sim, target, from + Vec3::new(0.0, 0.0, -d));
    // Settle, and finish drawing what the last kill gave.
    sim.seconds(1.3);
}

/// The party's gun game (`test:gun_game_hl2dm`) with `ladder` and every
/// game's knife.
pub(crate) fn ladder_text(ladder: &[&str], rung_extras: bool) -> String {
    let text = originals::MINIGAMES.iter().find(|(f, _)| f.contains("gun_game_dust2")).unwrap().1;
    let start = text.find("ladder = [").unwrap();
    let end = start + text[start..].find(']').unwrap() + 1;
    let ladder = format!("ladder = [{}]", ladder.iter().map(|g| format!("\"{g}\"")).collect::<Vec<_>>().join(", "));
    let text = format!("{}{}{}", &text[..start], ladder, &text[end..]);
    text.replace("originals:gun_game_dust2", "test:gun_game_hl2dm")
        .replace("weapons = [\"cs_source\", \"combat_arms\"]", "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]")
        .replace(
            "kit = { cs_source = [\"cs_source:weapon_knife\"], combat_arms = [\"combat_arms:m9_knife\"] }",
            "kit = { cs_source = [\"cs_source:weapon_knife\"], hl2dm = [\"hl2dm:weapon_crowbar\"], combat_arms = [\"combat_arms:m9_knife\"] }",
        )
        .replace("rung_extras = true", &format!("rung_extras = {rung_extras}"))
}

pub(crate) fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<chimera_sim::minigame::ladder::Ladder>().clone();
    chimera_sim::minigame::ladder::set_rung(w, e, r, &l);
}
