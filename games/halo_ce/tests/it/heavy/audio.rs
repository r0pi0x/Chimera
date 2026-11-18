//! Halo's lasting sounds on the user's disc image (`game::audio`,
//! `game::maps::audio`): every multiplayer map's sound scenery against
//! scenario.md's counts (8 on Beaver Creek, 2 on Blood Gulch, 3 on
//! Damnation, none elsewhere) and their looping sounds, the background
//! sounds and environments each map carries, Blood Gulch's background
//! (sounds.md: `bloodgultch`, one track looping `countryday2`, fades of
//! 3 s) heard everywhere in the level, its teleporter loop (0.5 and 2.5
//! WU, class 50), the rocket's flight loop, the teleporter's activate
//! sound, the plasma pistol's charge hum and the Spartan's footsteps.
//! Each test passes with a note without an install.

use bevy::math::Vec3;
use halo_ce::game::{
    MULTIPLAYER_MAPS,
    audio::library::{AudioLibrary, ENTITY_CLASS, KEY},
};

use super::maps::{load, mount};

fn library(map: &chimera_world::map::MapData) -> AudioLibrary {
    let e = map.entities.iter().find(|e| e.classname() == ENTITY_CLASS).expect("the audio entity");
    AudioLibrary::decode(e.get(KEY).unwrap()).expect("it decodes")
}

/// The sound scenery per map (scenario.md) and what every map carries.
#[test]
fn every_maps_sounds() {
    let Some(mount) = mount() else { return };
    for name in MULTIPLAYER_MAPS {
        let map = load(&mount, name, false);
        let lib = library(&map);
        let level = lib.level.as_ref().unwrap();
        let want = match *name {
            "beavercreek" => 8,
            "bloodgulch" => 2,
            "damnation" => 3,
            _ => 0,
        };
        assert_eq!(level.scenery.len(), want, "{name}: sound scenery");
        let backgrounds = level.backgrounds.iter().filter(|b| !b.is_empty()).count();
        eprintln!(
            "{name}: {} clusters, {backgrounds} background sounds {:?}, {} environments, {} zones, {} tree nodes, {} sound scenery, {} looping sounds",
            level.clusters,
            level.backgrounds,
            level.environments.len(),
            level.zones.len(),
            level.tree.len(),
            level.scenery.len(),
            lib.loops.len()
        );
        // Every looping sound the level names exists with a loop entry
        // whose clip is in the map's sounds.
        for s in level.backgrounds.iter().filter(|b| !b.is_empty()).chain(level.scenery.iter().map(|s| &s.sound)) {
            let l = &lib.loops[s];
            for t in &l.tracks {
                let entry = &lib.sounds[&t.looped].loop_entry;
                let e = map.sounds.entry(entry).unwrap_or_else(|| panic!("{name}: no entry {entry}"));
                assert!(!map.sounds.clips[e.waves[0]].samples.is_empty());
            }
        }
        // Every point of the drawn level is in a zone.
        assert!(!level.zones.is_empty(), "{name}");
        // The kit: the footsteps (6 kinds of 32 or 33 materials).
        assert_eq!(lib.footsteps.len(), 6, "{name}");
        assert!(lib.footsteps[1].iter().any(|s| s.ends_with("run")), "{name}: {:?}", lib.footsteps[1]);
    }
}

/// Blood Gulch: its background loop by the spec, the teleporter loop.
#[test]
fn blood_gulchs_sounds() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let lib = library(&map);
    let level = lib.level.as_ref().unwrap();
    let bg = r"sound\sfx\ambience\multiplayer\bloodgultch";
    assert_eq!(level.backgrounds, [bg]);
    let l = &lib.loops[bg];
    assert_eq!(l.tracks.len(), 1);
    let t = &l.tracks[0];
    assert_eq!(t.looped, r"sound\sfx\ambience\multiplayer\countryday2");
    assert_eq!((t.fade_in, t.fade_out, t.gain), (3.0, 3.0, 1.0));
    let amb = &lib.sounds[&t.looped];
    assert!(amb.stereo && amb.dry());
    // The loop clip: every chain once (21 chunks: 4 x 65 520 bytes ... ;
    // countryday2's chains total 1 141 560 bytes of 72-byte blocks).
    let clip = &map.sounds.clips[map.sounds.entry(&amb.loop_entry).unwrap().waves[0]];
    assert_eq!((clip.rate, clip.channels), (44_100, 2));
    eprintln!("countryday2 loop: {:.2} s", amb.loop_seconds);
    // The listener anywhere in the level hears it: the first spawn.
    let p = map.spawns[0].0 + Vec3::Y;
    let zone = level.zone(halo_ce::game::maps::space::to_game(p)).unwrap();
    assert_eq!(level.background(zone), Some(bg));
    // The teleporters' loops.
    let tele = r"sound\sfx\ui\teleporter_loop";
    assert!(level.scenery.iter().all(|s| s.sound == tele));
    let lt = &lib.loops[tele];
    let s = &lib.sounds[&lt.tracks[0].looped];
    assert_eq!((s.min, s.max, s.class, s.stereo), (0.5, 2.5, 50, false));
    // The rocket's flight loop (its projectile's attachment).
    eprintln!("projectile loops: {:?}", lib.projectiles);
    eprintln!("weapon loops: {:?}", lib.weapons);
    assert!(lib.projectiles.keys().any(|p| p.contains("rocket")), "{:?}", lib.projectiles);
    // The teleporter's activate sound, for the teleported player.
    let activate = halo_ce::game::fx::name_of(halo_ce::game::maps::teleporters::ACTIVATE_SOUND);
    assert!(map.sounds.entry(&activate).is_some_and(|e| !e.waves.is_empty()), "{activate}");
}

/// The plasma pistol's charge hum (playtest 2026-11-18: no charge sound):
/// its weapon tag's looping attachment `plasma rifle\charging` scaled by
/// function D, which follows the primary charge (weapons.md 17, export B
/// = 10): silent at rest, full when charged, and played by
/// `audio::attached::weapons` at that scale.
#[test]
fn the_plasma_pistols_charge_hums_by_its_charge() {
    use halo_ce::game::{
        audio::{attached::weapon_loop_scale, library::WeaponLoop},
        view::displays::{Functions, export, step},
        weapons::{guns, tick::TriggerMode, HaloGun},
    };
    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let lib = library(&map);
    let pp = r"weapons\plasma pistol\plasma pistol";
    let loops = &lib.weapons[pp];
    assert!(loops.contains(&WeaponLoop { sound: r"sound\sfx\weapons\plasma rifle\charging".into(), scale: 4 }), "{loops:?}");
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    let file = blam_formats::cache::MapFile::load(&bytes).unwrap();
    let shows = halo_ce::game::view::load::displays_of(&file);
    let (id, gun) = guns().into_iter().find(|(_, g)| g.tag == pp).unwrap();
    let program = &shows.guns[id.as_str()];
    let scale = |trigger: TriggerMode| {
        let state = HaloGun { trigger, ..Default::default() };
        let mut f = Functions::default();
        step(program, &|code| export(code, &gun, &state), None, &shows.tables, &mut f);
        weapon_loop_scale(4, &f.outgoing)
    };
    assert_eq!(scale(TriggerMode::Idle), 0.0, "silent at rest");
    assert!((scale(TriggerMode::Charged) - 1.0).abs() < 1e-5, "full when charged");
}

/// What a machine's sound seams were asked for.
#[derive(bevy::prelude::Resource, Default)]
struct Heard {
    shots: Vec<chimera_world::map::PlaySound>,
    starts: Vec<chimera_world::map::StartSound>,
}

fn record(
    mut shots: bevy::prelude::MessageReader<chimera_world::map::PlaySound>,
    mut controls: bevy::prelude::MessageReader<chimera_world::map::SoundControl>,
    mut heard: bevy::prelude::ResMut<Heard>,
) {
    heard.shots.extend(shots.read().cloned());
    for c in controls.read() {
        if let chimera_world::map::SoundControl::Start(s) = c {
            heard.starts.push(s.clone());
        }
    }
}

/// Item 6 of the brief's check: the server and two clients in one process
/// on Blood Gulch; client 1's player walks beside client 0's. Client 0's
/// machine plays the level's ambience (the background loop) and the
/// remote player's footsteps at that player, from replicated state alone.
#[test]
fn a_client_hears_the_ambience_and_a_remote_players_steps() {
    use bevy::prelude::*;
    use chimera_core::core::Intent;
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use chimera_world::map::MapSimPlugin;
    use halo_ce::harness::HaloSim;
    use std::time::Duration;

    let Some(mount) = mount() else { return };
    let map = load(&mount, "bloodgulch", false);
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 3, 2, move |app: &mut App| {
        app.add_plugins(MapSimPlugin::new(map.clone()))
            .init_resource::<Heard>()
            .add_systems(Last, record);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(900);
    sim.ticks(30);
    // Client 1's player beside client 0's, on the server.
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let here = sim.server.app.world().get::<Transform>(a).unwrap().translation;
    sim.server.app.world_mut().get_mut::<Transform>(b).unwrap().translation = here + Vec3::X * 2.0;
    sim.ticks(30);
    let walker = sim.local_player(1).unwrap();
    sim.clients[0].app.world_mut().resource_mut::<Heard>().shots.clear();
    for _ in 0..120 {
        sim.clients[1].app.world_mut().get_mut::<Intent>(walker).unwrap().move_axis = Vec2::Y;
        sim.step();
    }
    let heard = sim.clients[0].app.world_mut().remove_resource::<Heard>().unwrap();
    let ambience: Vec<&str> = heard.starts.iter().map(|s| s.entry.as_str()).filter(|e| e.contains("countryday2")).collect();
    assert!(!ambience.is_empty(), "client 0 starts the background loop: {:?}", heard.starts.iter().map(|s| &s.entry).collect::<Vec<_>>());
    let steps: Vec<&chimera_world::map::PlaySound> = heard.shots.iter().filter(|s| s.entry.contains("footsteps")).collect();
    eprintln!("client 0 heard {} steps: {:?}", steps.len(), steps.iter().map(|s| (&s.entry, s.at)).take(6).collect::<Vec<_>>());
    // Four seconds of running: a step about every 0.3 to 0.5 s.
    assert!(steps.len() >= 4, "{}", steps.len());
    // At the walker as client 0 shows it (within a couple of metres).
    let theirs = sim.clients[0]
        .app
        .world_mut()
        .query_filtered::<(Entity, &Transform), (With<Intent>, Without<chimera_core::core::LocalPlayer>)>()
        .iter(sim.clients[0].app.world())
        .map(|(_, t)| t.translation)
        .collect::<Vec<_>>();
    let last = steps.last().unwrap().at.unwrap();
    assert!(theirs.iter().any(|p| p.distance(last) < 3.0), "{last} against {theirs:?}");
}
