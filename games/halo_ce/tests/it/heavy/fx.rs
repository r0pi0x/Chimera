//! Halo's weapon effects library (`game::fx::load::compile`) from the
//! user's disc image: the four guns' firing, empty and impact effects
//! compile with their sounds decoded (every entry has clips, the levels
//! follow the classes' minimum distances), the first-person graph's
//! sounds by slot, the muzzle lights (effects.md "The muzzle flash
//! light"), the assault rifle's tracer (its contrail's 0.05 WU state) and
//! the plasma bolt's light; and a map dressed with it carries the sound
//! entries, particle materials and decal groups its effects name. Passes
//! with a note without an install.

use blam_formats::cache::MapFile;
use halo_ce::game::fx::load::{FxPartKind, compile, gun_tags};

fn bloodgulch() -> Option<MapFile> {
    let mount = super::maps::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

#[test]
fn the_four_guns_effects_compile() {
    let Some(map) = bloodgulch() else { return };
    let tags = gun_tags();
    let set = compile(&map, &tags.iter().map(String::as_str).collect::<Vec<_>>(), false);
    assert!(set.warnings.is_empty(), "{:?}", set.warnings);
    let lib = &set.library;
    assert_eq!(lib.guns.len(), halo_ce::game::weapons::GUNS.len(), "every gun of the arsenal has its effects");
    let ar = lib.gun("weapons\\assault rifle\\assault rifle").unwrap();
    for (name, p) in &ar.fp_markers {
        eprintln!("assault rifle first-person marker {name}: {p:?}");
    }
    let fire = &lib.effects[ar.firing[0].unwrap()];
    assert_eq!(fire.path, "weapons\\assault rifle\\effects\\fire bullet");
    assert_eq!(fire.events.len(), 4);
    let sounds: Vec<&str> = fire.events[1].parts.iter().filter_map(|p| match &p.kind {
        FxPartKind::Sound(s) => Some(s.as_str()),
        _ => None,
    }).collect();
    assert_eq!(sounds, ["halo_ce:sound\\sfx\\weapons\\assault rifle\\fire", "halo_ce:sound\\sfx\\weapons\\pistol\\eject"]);
    // The fire sound: 4 permutations, gain 0.5, class weapon fire (4 WU).
    let e = &set.entries["halo_ce:sound\\sfx\\weapons\\assault rifle\\fire"];
    assert_eq!(e.waves.len(), 4);
    assert!((e.volume.start - 0.5).abs() < 1e-6);
    assert!(matches!(e.level, chimera_world::map::sound::SoundLevel::Db(l) if (l - 82.5).abs() < 0.1), "{:?}", e.level);
    for (name, entry) in &set.entries {
        assert!(!entry.waves.is_empty(), "{name}");
    }
    // The graph's sounds: ready and reload.
    assert_eq!(ar.anim_sounds["ready"].0, "halo_ce:sound\\sfx\\weapons\\assault rifle\\weapon ready");
    assert_eq!(ar.anim_sounds["reload-empty"].0, "halo_ce:sound\\sfx\\weapons\\weapon_anims\\ar_reload");
    let (light, marker) = ar.light.clone().unwrap();
    assert_eq!(marker, "primary trigger");
    assert!((light.radius - 2.0).abs() < 1e-6 && (light.color[0] - 0.961).abs() < 1e-3);
    let tracer = ar.tracer.clone().unwrap();
    assert!((tracer.width - 0.05).abs() < 1e-6);
    // The AR's kick: 0.5 degree for one tick.
    let (rot, ticks) = ar.kick.unwrap();
    assert!((rot - 0.00873).abs() < 1e-4 && ticks == 1, "{rot} {ticks}");
    // Stone (2): the AR's stone impact with its decal and sound.
    let stone = &lib.effects[ar.impacts[2].unwrap()];
    assert_eq!(stone.path, "weapons\\assault rifle\\effects\\impact stone");
    let parts: Vec<&FxPartKind> = stone.events.iter().flat_map(|e| e.parts.iter().map(|p| &p.kind)).collect();
    assert!(parts.contains(&&FxPartKind::Decal("halo_ce:effects\\decals\\bullet holes\\stone".into())));
    assert!(parts.contains(&&FxPartKind::Sound("halo_ce:sound\\sfx\\impulse\\impacts\\granhit".into())));
    assert!(set.decals.iter().any(|(n, v)| n == "halo_ce:effects\\decals\\bullet holes\\stone" && !v.is_empty()));
    let pistol = lib.gun("weapons\\pistol\\pistol").unwrap();
    assert!(pistol.light.is_some() && pistol.tracer.is_none());
    let plasma = lib.gun("weapons\\plasma rifle\\plasma rifle").unwrap();
    assert_eq!(plasma.firing.len(), 2, "the two barrels");
    assert!(plasma.bolt_light.is_some() && plasma.tracer.is_some());
    assert!(plasma.anim_sounds.contains_key("overheating") && plasma.anim_sounds.contains_key("o-h-exit"));
    let shotgun = lib.gun("weapons\\shotgun\\shotgun").unwrap();
    assert!(shotgun.anim_sounds.contains_key("enter"), "{:?}", shotgun.anim_sounds.keys().collect::<Vec<_>>());
    // Every particle has its material.
    assert!(lib.particles.iter().all(|p| p.material.is_some()), "{:?}", lib.particles.iter().filter(|p| p.material.is_none()).map(|p| &p.path).collect::<Vec<_>>());
}
