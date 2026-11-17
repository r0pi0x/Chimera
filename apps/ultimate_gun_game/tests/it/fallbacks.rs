//! A game that lends only guns and movement (the test kit's stand-in,
//! `testkit::STAND_IN`, game `kit`) takes part in the automatic gun game:
//! its guns on the ladder, other games' maps in the pool, and for its
//! holder the fallback HUD (the base game's) and the fallback body (none
//! of its game among the map's models: the stand-in body).

use bevy::prelude::*;
use chimera_core::{
    core::LocalPlayer,
    game_parts::PartsRegistry,
    gameplay::Gameplay,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, auto, ladder::Ladder},
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{BodyModel, CharacterModels, MapCharacterModel, MapSimPlugin},
};
use combat_arms::{game::movement, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{BASE, MINIGAME, UltimateGunGamePlugin};

use crate::parts::{of, rungs};

/// The stand-in game, composed with the party's two.
struct StandIn;

impl Plugin for StandIn {
    fn build(&self, app: &mut App) {
        testkit::add_stand_in_game(app);
    }
}

/// Every gun of every game: the stand-in's are on the ladder whatever the
/// seed.
const ALL: usize = 64;

fn with_stand_in() -> Sim {
    let mut sim = Sim::new(
        CaSim,
        (
            GreyboxMapPlugin,
            MapSimPlugin::empty(),
            PartyPartsPlugin,
            StandIn,
            UltimateGunGamePlugin { seed: 3, rungs: ALL, ..Default::default() },
        ),
    );
    // The map's bodies: one of Combat Arms' (0), one of CS:S's (1).
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    sim.app
        .insert_resource(CharacterModels(std::sync::Arc::new(vec![body("combat_arms"), body("cs_source")])));
    sim
}

#[test]
fn a_guns_only_game_is_reported_and_falls_back() {
    let sim = with_stand_in();
    let report = auto::report(sim.app.world());
    let kit = report.iter().find(|c| c.game == "kit").expect("the stand-in's report");
    assert!(kit.playable() && kit.missing.is_empty(), "{}", kit.line());
    assert_eq!((kit.knife, kit.characters, kit.hud), (None, None, None));
    assert!(kit.maps.is_empty());
    let registry = sim.app.world().resource::<PartsRegistry>();
    let f = auto::fallbacks(registry, "kit").unwrap();
    assert_eq!(f, auto::Fallbacks { hud: BASE, own_body: false, own_maps: false });
    let ladder = rungs(sim.app.world());
    assert_eq!(of(&ladder, "kit"), 3, "its three guns: {ladder:?}");
    let def = sim.app.world().resource::<minigame::Minigames>().get(MINIGAME).unwrap();
    assert!(def.maps.iter().all(|m| !m.starts_with("kit:")) && !def.maps.is_empty(), "other games' maps: {:?}", def.maps);
    assert!(def.slots.weapons.iter().any(|w| w == "kit"));
    assert_eq!(def.spawn.kit.for_game(Some("kit")), ["combat_arms:m9_knife"], "no knife of its own: the base game's");
}

#[test]
fn its_holder_moves_as_it_with_the_fallback_hud_and_body() {
    let mut sim = with_stand_in();
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let _b = sim.spawn_character(Vec3::new(2.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    let rung_of = |game: &str| l.rungs.iter().position(|id| id.starts_with(&format!("{game}:"))).unwrap();
    // A CS:S gun first: CS:S's HUD and body.
    minigame::ladder::set_rung(sim.app.world_mut(), a, rung_of("cs_source"), &l);
    sim.seconds(1.2);
    let w = sim.app.world();
    assert_eq!(w.resource::<Gameplay>().hud, "cs_source");
    assert_eq!(w.get::<BodyModel>(a).map(|b| b.0), Some(1));
    // The stand-in's gun: its movement, the base game's HUD, no body (a
    // capsule).
    let kit = rung_of("kit");
    minigame::ladder::set_rung(sim.app.world_mut(), a, kit, &l);
    sim.seconds(1.2);
    let w = sim.app.world();
    assert_eq!(testkit::active_id(w, a), Some(l.rungs[kit]));
    assert_eq!(w.get::<GameIdentity>(a).map(|g| g.0.as_str()), Some("kit"));
    assert_eq!(w.get::<MovementSlot>(a).map(|m| m.0), Some(testkit::STAND_IN_MOVEMENT));
    assert_eq!(w.resource::<Gameplay>().hud, BASE, "the fallback HUD");
    assert_eq!(w.get::<BodyModel>(a), None, "the fallback body: the stand-in");
    // It still moves (the stand-in's movement steps it).
    sim.intent(a).move_axis = Vec2::Y;
    let from = sim.position(a);
    sim.seconds(0.5);
    assert!(sim.position(a).distance(from) > 0.5, "it walks");
}
