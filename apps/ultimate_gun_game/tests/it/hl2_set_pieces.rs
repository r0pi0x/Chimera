//! HL2:DM's set pieces work in Ultimate Gun Game (hl2dm/maps_entities.md 8
//! and 16): `hl2dm::party_sim` adds `SetPiecesPlugin`, so dm_runoff's
//! headcrab canisters fly, land and fire their outputs on the party's
//! simulation, and the AI sound list that dm_steamlab's gas trap
//! (`env_microphone`) listens to is fed.

use bevy::prelude::*;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::entities::{FireKeyedOutput, MapEntityInput, MapEntitySpawned},
};
use combat_arms::harness::CaSim;
use hl2dm::game::set_pieces::SetPiecesPlugin;
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::UltimateGunGamePlugin;

#[derive(Resource, Default)]
struct Fired(Vec<String>);

fn listen(mut keyed: MessageReader<FireKeyedOutput>, mut fired: ResMut<Fired>) {
    fired.0.extend(keyed.read().map(|k| k.output.clone()));
}

fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn a_headcrab_canister_lands_in_ultimate_gun_game() {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin { seed: 3, rungs: 8, ..Default::default() }));
    assert!(sim.app.is_plugin_added::<SetPiecesPlugin>(), "the party composes the set pieces");
    sim.app.init_resource::<Fired>().add_systems(FixedLast, listen);
    sim.ticks(3);
    // dm_runoff's canister, fired by an input (a short flight, as the map's
    // one is 4 s).
    let w = sim.app.world_mut();
    w.write_message(MapEntitySpawned {
        key: 7,
        map_index: None,
        class: "env_headcrabcanister".into(),
        keyvalues: kv(&[("FlightSpeed", "3000"), ("FlightTime", "1"), ("Damage", "150"), ("DamageRadius", "350"), ("spawnflags", "565248")]),
        origin: Vec3::new(0.0, 0.0, 8.0),
        angles: Vec3::new(-71.0, 0.0, 0.0),
    });
    sim.ticks(2);
    let w = sim.app.world_mut();
    w.write_message(MapEntityInput { key: 7, map_index: None, input: "firecanister".into(), value: String::new(), activator: None });
    sim.ticks(10);
    assert_eq!(sim.app.world().resource::<Fired>().0.iter().filter(|o| *o == "OnLaunched").count(), 1, "launched");
    assert!(!sim.app.world().resource::<Fired>().0.iter().any(|o| o == "OnImpacted"), "still in the air");
    sim.seconds(1.5);
    assert_eq!(sim.app.world().resource::<Fired>().0.iter().filter(|o| *o == "OnImpacted").count(), 1, "landed");
}
