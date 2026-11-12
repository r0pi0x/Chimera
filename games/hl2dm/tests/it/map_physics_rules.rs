//! HL2:DM's physics props' rules on its maps only, in a game of several
//! (Ultimate Gun Game's case: `game::party_sim` on Chimera's base;
//! specs/hl2dm/source_deltas.md 1.5, gravity_gun.md Corrections 1, 2, 9;
//! `game::physics::MapPhysicsRules`):
//! with an HL2:DM map loaded (the map's game, `map::impact::ActiveImpacts`)
//! players take props' impacts at energy scale 4, a multiplayer prop
//! collides with them and a prop running into one knocks him; on another
//! game's map the rules are that game's again (scale 1, CS:S's walk-through
//! multiplayer props, no knock). The maps here are stand-ins: the rules
//! follow the map's game, not its file.

use bevy::prelude::*;
use chimera_sim::SimBasePlugins;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        PhysicsProp, PushAway,
        impact::{ActiveImpacts, MapImpacts},
    },
};
use source_engine::source_movement::shadow::ShadowKnock;
use source_logic::logic::{KeepPropsOnBreak, PlayerImpactScale};

/// Chimera's base with HL2:DM's parts as a game of several takes them.
fn party() -> Sim {
    Sim::new(SimBasePlugins, (GreyboxMapPlugin, hl2dm::game::party_sim))
}

/// The loaded map is `game`'s.
fn map_of(sim: &mut Sim, game: &str) {
    sim.app.world_mut().insert_resource(ActiveImpacts(MapImpacts {
        game: Some(game.into()),
        looks: vec![game.into()],
    }));
    sim.ticks(2);
}

/// A multiplayer prop as CS:S's rule leaves it (solid to players by its
/// pushaway), placed now.
fn multiplayer_prop(sim: &mut Sim) -> Entity {
    let e = sim
        .app
        .world_mut()
        .spawn((
            PhysicsProp {
                push: PushAway::Solid,
                mass: 20.0,
                bounds: (Vec3::splat(-0.3), Vec3::splat(0.3)),
            },
            Transform::from_xyz(5.0, 1.0, 5.0),
        ))
        .id();
    sim.ticks(1);
    e
}

fn push(sim: &Sim, e: Entity) -> PushAway {
    sim.app.world().get::<PhysicsProp>(e).unwrap().push
}

fn impact_scale(sim: &Sim) -> f32 {
    sim.app.world().resource::<PlayerImpactScale>().0
}

#[test]
fn hl2_maps_props_follow_hl2s_rules_and_others_their_own() {
    let mut sim = party();
    map_of(&mut sim, "cs_source");
    let before = impact_scale(&sim);
    assert_eq!(before, 1.0, "CS:S's players' impact scale");
    assert!(!sim.app.world().contains_resource::<ShadowKnock>(), "no knock on a CS:S map");
    let cs_prop = multiplayer_prop(&mut sim);
    assert_eq!(push(&sim, cs_prop), PushAway::Solid, "CS:S's multiplayer prop keeps its pushaway");

    map_of(&mut sim, "hl2dm");
    assert_eq!(impact_scale(&sim), 4.0, "HL2:DM's players take impacts at scale 4 (1.5)");
    assert!(sim.app.world().contains_resource::<ShadowKnock>(), "a prop knocks a player");
    assert_eq!(sim.app.world().get_resource::<KeepPropsOnBreak>().map(|k| k.0), Some(true), "Corrections 2");
    let hl2_prop = multiplayer_prop(&mut sim);
    assert_eq!(push(&sim, hl2_prop), PushAway::Collide, "HL2:DM's multiplayer props collide with players (Corrections 1)");

    // Back on another game's map: its rules again.
    map_of(&mut sim, "combat_arms");
    assert_eq!(impact_scale(&sim), before, "the other game's scale is back");
    assert!(!sim.app.world().contains_resource::<ShadowKnock>());
    assert!(sim.app.world().get_resource::<KeepPropsOnBreak>().is_none_or(|k| !k.0));
    let other = multiplayer_prop(&mut sim);
    assert_eq!(push(&sim, other), PushAway::Solid);
}
