//! TF2's guns' looks on another game's map, from the user's install:
//! `maps::add_guest_looks` puts TF2's own particle systems (the
//! explosion, every muzzle flash, both teams' flames) and its impact
//! looks onto a map of no TF2 at all (before, de_dust2 showed no flash, no
//! flame and Source's grey puff for the rocket), the route then gives a
//! TF2 gun's impacts to TF2 there, not the map's game or the app's base
//! game, and the `players` set, offered with every TF2 match, holds the
//! files they read (the particle files, the class portraits). Passes with
//! a note without the install.

use chimera_sim::weapon::impact::{ImpactBy, impact_by};
use chimera_world::map::{MapData, impact::ActiveImpacts};
use source_engine::content::material::MaterialLoader;
use tf2::game::{maps, sets, weapons::looks};

use super::maps::install_mount;

/// A map of another game (CS:S's, as de_dust2 is) with TF2's looks on it.
fn dressed() -> Option<MapData> {
    let mount = install_mount()?;
    let mut map = MapData::default();
    map.impacts.game = Some("cs_source".into());
    map.impacts.add("cs_source");
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    maps::add_guest_looks(&mut materials, &mut map);
    Some(map)
}

#[test]
fn tf2s_effects_load_onto_another_games_map() {
    let Some(map) = dressed() else { return };
    let systems = &map.particle_systems;
    for name in looks::particle_systems().iter().copied() {
        let i = systems.find(name).unwrap_or_else(|| panic!("{name}: not loaded ({:?})", map.warnings));
        let d = &systems.defs[i];
        assert!(d.material.is_some() || !d.children.is_empty(), "{name}: nothing to draw");
    }
    assert!(systems.scale > 0.0);
    // Every system a shot starts is there.
    let gun_looks = looks::GunLooks::default();
    for gun in tf2::game::weapons::GUNS {
        if let Some(name) = gun_looks.muzzle_of(gun) {
            assert!(systems.find(name).is_some(), "{gun}: {name}");
        }
    }
    assert!(!map.kit.decals.decals.is_empty(), "TF2's impact decals");
}

/// The flame stream's core as our loader reads it from the install
/// (specs/tf2/flames.md 3.3): its sprites take the newest point, live
/// its duration, ride it, their radius 23 x (0.5 to 1 over 64 units from
/// the muzzle) and their tint (0.2, 0.2, 1) to white over 200 units.
#[test]
fn the_flame_cores_operators_load() {
    use chimera_world::map::psys::{Field, Init, Op};
    let Some(map) = dressed() else { return };
    let systems = &map.particle_systems;
    let notes: Vec<&String> = map.warnings.iter().filter(|w| w.contains("particle operator")).collect();
    eprintln!("particle notes: {notes:?}");
    let core = &systems.defs[systems.find("new_flame_core").expect("new_flame_core")];
    assert!(core.initializers.contains(&Init::AssignTargetCp), "{:?}", core.initializers);
    assert!(core.initializers.contains(&Init::LifetimeFromCp), "{:?}", core.initializers);
    assert!(core.operators.iter().any(|o| matches!(o, Op::FollowCp { catch_up } if (*catch_up - 100.0).abs() < 1e-3)), "{:?}", core.operators);
    let radius = core.operators.iter().find_map(|o| match *o {
        Op::RemapDistance { field: Field::Radius, near, far, low, high, scale_initial, .. } => Some((near, far, low.x, high.x, scale_initial)),
        _ => None,
    });
    assert_eq!(radius, Some((0.0, 64.0, 0.5, 1.0, true)), "{:?}", core.operators);
    let tint = core.operators.iter().find_map(|o| match *o {
        Op::RemapDistance { field: Field::Tint, near, far, low, high, .. } => Some((near, far, low, high)),
        _ => None,
    });
    let (near, far, low, high) = tint.expect("the tint remap");
    assert!(near == 0.0 && far == 200.0 && (low - bevy::prelude::Vec3::new(0.2, 0.2, 1.0)).length() < 1e-3 && high == bevy::prelude::Vec3::ONE);
    assert!((core.radius - 23.0).abs() < 1e-3);
}

#[test]
fn a_tf2_guns_impacts_are_tf2s_on_another_games_map() {
    let Some(map) = dressed() else { return };
    let providers = ["combat_arms", "cs_source", "tf2"];
    let active = ActiveImpacts(map.impacts.clone());
    assert_eq!(impact_by(Some("tf2"), Some(&active), &providers), ImpactBy::Game("tf2"));
    // A CS:S gun's stay CS:S's.
    assert_eq!(impact_by(Some("cs_source"), Some(&active), &providers), ImpactBy::Game("cs_source"));
}

#[test]
fn the_players_set_holds_the_looks_and_portraits() {
    let Some(mount) = install_mount() else { return };
    let pieces = sets::player_pieces(&mount);
    let (_, players) = pieces.iter().find(|(n, _)| n == "players").expect("players");
    for f in looks::PARTICLE_FILES {
        assert!(players.iter().any(|p| p.eq_ignore_ascii_case(f)), "{f}");
    }
    for want in ["materials/hud/class_scoutred.vmt", "materials/hud/class_engiblue.vmt", "materials/hud/character_red_bg.vmt"] {
        assert!(players.iter().any(|p| p.eq_ignore_ascii_case(want)), "{want}");
    }
    assert!(players.iter().any(|p| p.to_lowercase().starts_with("sound/physics/") || p.to_lowercase().contains("impact")), "the impact sounds");
}
