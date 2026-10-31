//! Spawn protection seen (user, 2026-10-29): the party gun games protect
//! for 2 s, and a protected character's body draws half see-through
//! (`chimera_present::render::tint::fade_protected`, its materials blended
//! at alpha ~0.5) on the host and on every network client (from the
//! replicated `NetProtected`), opaque again (its own materials back)
//! about 0.2 s after the protection ends.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{SimClock, SpawnProtected};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_present::render::{
    bodies::CharacterBody,
    probe_lit::ProbeLit,
    prop_material::{PropMaterial, PropParams},
    tint::{apply_character_looks, fade_protected},
};
use chimera_sim::minigame::{self, Minigames, def::Protection};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::{
    bodies_drawn::{floor, render_assets},
    gun_game::soldier,
};

/// The opacity and blend of the first material under `character`'s
/// drawn models (`ProbeLit`), if it has one.
fn look(world: &mut World, character: Entity) -> Option<(f32, AlphaMode)> {
    let mut lit = world.query::<(Entity, &ProbeLit)>();
    let handle = lit
        .iter(world)
        .find(|(e, _)| {
            let mut at = Some(*e);
            while let Some(x) = at {
                if x == character {
                    return true;
                }
                at = world.get::<ChildOf>(x).map(ChildOf::parent);
            }
            false
        })
        .and_then(|(_, l)| l.materials.first().cloned())?;
    let m = world.resource::<Assets<PropMaterial>>().get(&handle)?;
    Some((m.params.base_color.w, m.alpha_mode))
}

fn see_through(l: Option<(f32, AlphaMode)>) -> bool {
    l.is_some_and(|(a, m)| (a - 0.5).abs() < 0.02 && m == AlphaMode::Blend)
}

fn opaque(l: Option<(f32, AlphaMode)>) -> bool {
    l.is_some_and(|(a, m)| a == 1.0 && m == AlphaMode::Opaque)
}

#[test]
fn the_party_gun_games_protect_for_two_seconds() {
    let sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    let all = sim.app.world().resource::<Minigames>();
    let ids: Vec<String> = all.all().filter(|d| d.id.contains("gun_game")).map(|d| d.id.clone()).collect();
    for def in all.all().filter(|d| d.id.contains("gun_game")) {
        assert_eq!(def.spawn.respawn.protection, Protection::both(2.0), "{}", def.id);
    }
    assert!(ids.iter().any(|i| i == DEFAULT_MINIGAME), "{ids:?}");
}

#[test]
fn a_protected_body_draws_half_see_through_and_fades_back_when_it_ends() {
    let mut sim = Sim::new(CaSim, (render_assets, chimera_present::MapPlugin::new(floor("test:protection")), PartyPartsPlugin));
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(0.5);
    let w = sim.app.world_mut();
    let now = w.resource::<SimClock>().now;
    let p = *w.get::<SpawnProtected>(a).expect("protected");
    assert!(p.bullets_until - now > 1.3, "2 s from the spawn: {p:?} at {now}");
    let has_body = w.get::<Children>(a).is_some_and(|c| c.iter().any(|c| w.get::<CharacterBody>(c).is_some()));
    assert!(has_body, "a drawn body");
    assert!(see_through(look(w, a)), "protected: {:?}", look(w, a));
    // Over (both windows): opaque again within about 0.2 s.
    for _ in 0..(5.0 * sim.tick_hz()) as u32 {
        if sim.app.world().get::<SpawnProtected>(a).is_none() {
            break;
        }
        sim.ticks(1);
    }
    assert!(sim.app.world().get::<SpawnProtected>(a).is_none(), "it ends");
    sim.seconds(0.05);
    let l = look(sim.app.world_mut(), a);
    assert!(l.is_some_and(|(a, _)| a > 0.5 && a < 1.0), "fading back: {l:?}");
    sim.seconds(0.25);
    assert!(opaque(look(sim.app.world_mut(), a)), "opaque: {:?}", look(sim.app.world_mut(), a));
}

/// Each app draws characters' looks (a client's headless stand-in for its
/// renderer: the same two systems the presentation runs).
fn looks(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
    render_assets(app);
    app.add_systems(PostUpdate, (fade_protected, apply_character_looks).chain());
}

/// A drawn model under `character` with its own opaque material.
fn draw(sim: &mut Sim, character: Entity) {
    let w = sim.app.world_mut();
    let m = w.resource_mut::<Assets<PropMaterial>>().add(PropMaterial {
        params: PropParams {
            base_color: Vec4::ONE,
            ..default()
        },
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });
    w.spawn((ProbeLit::new(vec![m], Vec3::ZERO), ChildOf(character)));
}

#[test]
fn a_network_client_draws_another_protected_player_see_through() {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        13,
        2,
        looks,
    );
    sim.set_frame(TICK_INTERVAL);
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    let b = sim.character_of(1).expect("client 1's player");
    let b_on_0 = {
        let id = sim.client_id(1);
        NetSim::owned_by(&mut sim.clients[0], Some(id)).expect("client 0's copy of it")
    };
    draw(&mut sim.clients[0], b_on_0);
    assert!(
        sim.until(600, |s| s.clients[0].app.world().get::<SpawnProtected>(b_on_0).is_some()),
        "client 0 learns b's protection"
    );
    sim.ticks(2);
    let l = look(sim.clients[0].app.world_mut(), b_on_0);
    assert!(see_through(l), "see-through on client 0: {l:?}");
    // The server's protection ends: opaque on the client too.
    assert!(sim.until(900, |s| s.server.app.world().get::<SpawnProtected>(b).is_none()), "over on the server");
    assert!(
        sim.until(300, |s| opaque(look(s.clients[0].app.world_mut(), b_on_0))),
        "opaque again on client 0: {:?}",
        look(sim.clients[0].app.world_mut(), b_on_0)
    );
}
