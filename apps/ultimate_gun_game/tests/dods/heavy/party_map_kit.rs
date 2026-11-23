//! A DoD:S map as the party loads it (another game's gameplay: its kit
//! read for players, `map::KitLoad::Players`, and kept by
//! `GameKit::stay`): all twelve class bodies stay, those worn by name too
//! (the Axis rifleman showed the US body when only the unnamed default
//! stayed), each with its weapon bones in its hands (dods' `bodies::HOLDS`), and
//! the guns' sound entries are in the map's table under DoD:S's names
//! (dods' `maps::own_sound`: the view models' draw, the scripts' shots). Passes
//! with a note without the install.
//! `cargo test -p ultimate_gun_game --features dev --test dods heavy::party_map_kit -- --nocapture`

/// DoD:S's id, and its class bodies' models (dods::game::bodies::CLASSES;
/// this crate links DoD:S only through the roster).
const GAME: &str = "dods";
const CLASSES: [&str; 12] = [
    "american_rifleman", "american_assault", "american_support", "american_sniper", "american_mg", "american_rocket",
    "german_rifleman", "german_assault", "german_support", "german_sniper", "german_mg", "german_rocket",
];

#[test]
fn a_dods_map_keeps_every_class_body_and_its_guns_sounds() {
    let data = match party_roster::load_map("dods:dod_avalanche", 0) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let ours: Vec<_> = data.kit.characters.iter().filter(|c| c.game == Some(GAME)).collect();
    assert_eq!(ours.len(), CLASSES.len(), "{:?}", ours.iter().map(|c| &c.name).collect::<Vec<_>>());
    for class in &CLASSES[1..] {
        let model = format!("models/player/{class}.mdl");
        assert!(ours.iter().any(|c| c.name.as_deref() == Some(model.as_str())), "{model} kept by name");
    }
    for c in &ours {
        let hand = c.bones.iter().position(|b| b.name == "ValveBiped.Bip01_R_Hand").unwrap();
        let weapon = c.bones.iter().find(|b| b.name == "ValveBiped.weapon_bone").unwrap();
        assert_eq!(weapon.parent, Some(hand));
        assert!(weapon.position.length() < 1.0, "{:?}: the weapon bone in the hand: {:?}", c.name, weapon.position);
    }
    for entry in ["Weapon_Thompson.Draw", "Weapon_Thompson.Shoot", "Weapon_Garand.ClipIn1", "Weapon_Thompson.WorldReload"] {
        let ours = format!("dods:{entry}").to_lowercase();
        assert!(data.sounds.entries.contains_key(&ours), "{ours} in the map's table");
    }
}

/// A practice target on a DoD:S map wears its class body and holds its
/// gun's world model (`bodies::hold_weapons`: `map::Held`).
#[test]
fn a_dods_target_holds_its_gun() {
    use bevy::prelude::*;
    use chimera_core::{console::Console, core::LocalPlayer};
    use chimera_sim::minigame::{self, targets::Target};
    use chimera_world::{
        harness::Sim,
        map::{BodyModel, Held, MapSimPlugin},
    };
    use combat_arms::{game::movement, harness::CaSim};
    use party_roster::PartyPartsPlugin;
    use ultimate_gun_game::{UltimateGunGamePlugin, sandbox::SANDBOX};
    let data = match party_roster::load_map("dods:dod_avalanche", 0) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let spawn = data.spawns[0].0;
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin, UltimateGunGamePlugin::default()));
    sim.ticks(2);
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(spawn + Vec3::Y * 0.1, movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(2.0);
    for line in ["target add dods:axis_rifleman 3", "target set all gun dods:weapon_k98"] {
        sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
        sim.ticks(3);
    }
    sim.seconds(2.0);
    let mut q = sim.app.world_mut().query_filtered::<(Option<&Held>, Option<&BodyModel>), With<Target>>();
    let got: Vec<_> = q.iter(sim.app.world()).map(|(h, b)| (h.cloned(), b.copied())).collect();
    assert_eq!(got.len(), 1);
    assert!(got[0].1.is_some(), "a body: {got:?}");
    assert_eq!(got[0].0.as_ref().and_then(|h| h.0.as_deref()), Some("dods:weapon_k98"), "{got:?}");
}
