//! Every living player is drawn (playtest 2026-10-28, "a bot stays
//! invisible", as either game's character): the gun game on the party's
//! simulation with the presentation's map (`chimera_present::MapPlugin`:
//! bodies drawn, ragdolls taking them over), ten players killing each other
//! at random through the rungs' identity swaps (CS:S's body and Combat
//! Arms'), deaths with ragdolls, respawns and map changes (some made while
//! players lie dead). After each step every living player has one drawn
//! body, its identity's game's, with meshes, nothing on its way hidden and
//! on the main camera's layer, and no `Ragdolled` mark left over. A body
//! may lag a swap or a respawn by a few ticks, never longer.
//!
//! The cause found: a player dead at a map change (the gun game's last
//! victim: the winning shot ends the match, the next map loads) lost its
//! ragdoll with the old map but kept `Ragdolled`, so it never got a body
//! again (`chimera_world::map::ragdoll`'s `remove_ragdolls` now clears it).

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, God, Hitgroup},
    slots::GameIdentity,
};
use chimera_sim::minigame::{self, ladder::Rung};
use chimera_world::{
    harness::Sim,
    map::{
        MapData, MapDebugView, change_map,
    },
};
use combat_arms::harness::CaSim;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};

use chimera_present::render::bodies::CharacterBody;

use crate::play::{place, soldier};
use crate::bodies::*;


#[test]
fn every_living_player_stays_drawn_through_swaps_deaths_and_map_changes() {
    let mut sim = Sim::new(CaSim, (render_assets, chimera_present::MapPlugin::new(floor("test:bodies_a")), PartyPartsPlugin));
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let players: Vec<Entity> = (0..10).map(|_| soldier(&mut sim)).collect();
    sim.seconds(1.0);
    let mut dice = Dice(0x5eed_1e55);
    let mut failing: HashMap<Entity, (u32, String)> = HashMap::new();
    let (mut kills, mut swaps, mut map_changes, mut dead_at_change) = (0, 0, 0, 0);
    let mut last_identity: HashMap<Entity, String> = HashMap::new();
    // About 90 s of play: a kill every 0.2 s when someone can be killed.
    for step in 0..450u32 {
        // Spread out, each on its own spot (spawns may stack them).
        for (i, p) in players.iter().enumerate() {
            if step % 20 == 0 && alive(sim.app.world(), *p) {
                place(&mut sim, *p, Vec3::new((i % 5) as f32 * 4.0 - 8.0, 1.0, (i / 5) as f32 * 4.0 - 2.0));
            }
        }
        let world = sim.app.world();
        let open: Vec<Entity> = players.iter().copied().filter(|p| alive(world, *p) && world.get::<God>(*p).is_none()).collect();
        if open.len() >= 2 {
            let a = open[dice.below(open.len())];
            let v = loop {
                let v = open[dice.below(open.len())];
                if v != a {
                    break v;
                }
            };
            let at = sim.position(v);
            sim.app.world_mut().write_message(Damage {
                target: v,
                attacker: Some(a),
                amount: 1000.0,
                point: at + Vec3::Y * 0.5,
                dir: Vec3::NEG_Z,
                hitgroup: Hitgroup::Chest,
                kind: DamageKind::Bullet,
                weapon: None,
                force: Vec3::ZERO,
            });
            kills += 1;
        }
        // A map change every 15 s, right after a kill (its victim dead).
        if step % 75 == 74 {
            sim.ticks(2);
            let world = sim.app.world();
            dead_at_change += players.iter().filter(|p| !alive(world, **p)).count();
            let name = if map_changes % 2 == 0 { "test:bodies_b" } else { "test:bodies_a" };
            change_map(sim.app.world_mut(), floor(name), MapDebugView::Normal);
            map_changes += 1;
        }
        // 0.2 s, checking every tick.
        let ticks = (0.2 * sim.tick_hz()) as u32;
        for _ in 0..ticks {
            sim.ticks(1);
            let world = sim.app.world_mut();
            for p in &players {
                if let Some(id) = world.get::<GameIdentity>(*p).map(|i| i.0.clone())
                    && last_identity.insert(*p, id.clone()).is_some_and(|old| old != id)
                {
                    swaps += 1;
                }
                let why = if alive(world, *p) { undrawn(world, *p) } else { None };
                match why {
                    Some(why) => {
                        let f = failing.entry(*p).or_insert((0, String::new()));
                        f.0 += 1;
                        f.1 = why;
                        let rung = world.get::<Rung>(*p).map(|r| r.0);
                        assert!(
                            f.0 <= 6,
                            "step {step}: living {p} (rung {rung:?}) undrawn for {} ticks: {}",
                            f.0,
                            f.1
                        );
                    }
                    None => {
                        failing.remove(p);
                    }
                }
            }
        }
    }
    let stats = format!("{kills} kills, {swaps} identity swaps, {map_changes} map changes, {dead_at_change} dead at them");
    assert!(kills > 60 && swaps > 30 && map_changes >= 5 && dead_at_change >= map_changes, "{stats}");
    eprintln!("{stats}");
}

/// The floor with HL2:DM's bodies too: its default (unnamed, the Combine
/// soldier's) and each of its 19 player models by name, as its guest
/// dressing loads them (`hl2dm::game::bodies`).
fn floor_with_hl2dm(name: &str) -> MapData {
    let mut map = floor(name);
    let default = hl2dm::game::weapons::inventory::DEFAULT_MODEL;
    for path in hl2dm::game::bodies::PLAYER_MODELS {
        let mut c = character("hl2dm");
        if path != default {
            c.name = Some(path.to_string());
        }
        map.kit.characters.push(c);
    }
    map
}

/// HL2:DM's rungs draw their model (spec player_models.md; the rung's
/// `Hl2Model`, worn by name): on the mixed ladder (`hl2dm_parts::LADDER`,
/// rung extras on), each living player on an HL2:DM rung has one drawn
/// body, HL2:DM's, the rung's model by name (the default when it has
/// none); a CS:S or Combat Arms rung gives
/// that game's body back; deaths leave ragdolls and the body comes back.
#[test]
fn hl2dm_rungs_draw_their_model() {
    use hl2dm::game::weapons::inventory::Hl2Model;
    let text = crate::mixed::mixed_text(true);
    let mut sim = Sim::new(
        CaSim,
        (render_assets, chimera_present::MapPlugin::new(floor_with_hl2dm("test:bodies_hl2")), PartyPartsPlugin),
    );
    crate::play::no_finale(sim.app.world_mut());
    minigame::MinigameAppExt::add_minigames(&mut sim.app, &[("hl2dm.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    let players: Vec<Entity> = (0..4).map(|_| soldier(&mut sim)).collect();
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<minigame::ladder::Ladder>().clone();
    let mut seen_hl2 = 0;
    for step in 0..12usize {
        for (i, p) in players.iter().enumerate() {
            minigame::ladder::set_rung(sim.app.world_mut(), *p, (step + i) % ladder.rungs.len(), &ladder);
            place(&mut sim, *p, Vec3::new(i as f32 * 4.0 - 6.0, 1.0, 0.0));
        }
        sim.seconds(1.2);
        let world = sim.app.world_mut();
        for p in &players {
            let id = world.get::<GameIdentity>(*p).map(|i| i.0.clone());
            let kids: Vec<Entity> = world.get::<Children>(*p).map(|c| c.to_vec()).unwrap_or_default();
            let bodies: Vec<usize> = kids.iter().filter_map(|c| world.get::<CharacterBody>(*c).map(|b| b.model)).collect();
            assert_eq!(bodies.len(), 1, "step {step}: {p} ({id:?}) has {} bodies", bodies.len());
            let models = world.resource::<chimera_world::map::CharacterModels>().clone();
            let m = &models.0[bodies[0]];
            assert_eq!(m.game.map(str::to_string), id, "step {step}: {p} wears another game's body");
            if id.as_deref() == Some("hl2dm") {
                seen_hl2 += 1;
                let want = world.get::<Hl2Model>(*p).map(|h| h.0.clone()).filter(|h| h != hl2dm::game::weapons::inventory::DEFAULT_MODEL);
                assert_eq!(m.name, want, "step {step}: {p} wears its rung's model");
            }
        }
    }
    assert!(seen_hl2 >= 12, "{seen_hl2} HL2:DM bodies checked");
    // A death leaves HL2:DM's ragdoll; the body comes back with the
    // respawn and the ragdoll stays (its rules).
    let p = players[0];
    minigame::ladder::set_rung(sim.app.world_mut(), p, 0, &ladder);
    sim.seconds(1.2);
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(players[1]),
        amount: 1000.0,
        point: at + Vec3::Y * 0.5,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::NEG_Z * 10.0,
    });
    sim.ticks(3);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&chimera_world::map::Ragdoll>().iter(world).count(), 1);
    sim.seconds(6.0);
    let world = sim.app.world_mut();
    assert!(alive(world, p), "respawned");
    assert!(undrawn_any(world, p).is_none(), "{:?}", undrawn_any(world, p));
}

