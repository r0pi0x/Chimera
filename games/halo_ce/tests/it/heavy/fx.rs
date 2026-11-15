//! Halo's weapon effects library (`game::fx::load::compile`) from the
//! user's disc image: the four guns' firing, empty and impact effects
//! compile with their sounds decoded (every entry has clips, the levels
//! follow the classes' minimum distances), the first-person graph's
//! sounds by slot, the muzzle lights (effects.md "The muzzle flash
//! light"), the assault rifle's tracer (its contrail's 0.05 WU state) and
//! the plasma bolt's light; and a map dressed with it carries the sound
//! entries, particle materials and decal groups its effects name. Passes
//! with a note without an install. The rest of the arsenal: each gun's
//! firing effect, the projectiles' looks and the three explosions, the
//! pickup sounds, and the grenades' and items' models.

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

/// The rest of the arsenal (effects.md "The rest of the arsenal (compact)",
/// "Grenades"): each gun's firing effect by path, the projectiles' impacts,
/// the three explosions (the rocket's, the frag's, the plasma grenade's),
/// the supercombine's and the plasma grenade's countdown; the rocket's and
/// needle's contrails, the grenades' lights in flight; the pickup sounds of
/// the equipment table (objects.md "Equipment").
#[test]
fn the_arsenals_effects_and_explosions_compile() {
    let Some(map) = bloodgulch() else { return };
    let tags = gun_tags();
    let set = compile(&map, &tags.iter().map(String::as_str).collect::<Vec<_>>(), false);
    assert!(set.warnings.is_empty(), "{:?}", set.warnings);
    let lib = &set.library;
    let path = |i: Option<usize>| i.map(|i| lib.effects[i].path.clone()).unwrap_or_default();
    for (gun, fire) in [
        (r"weapons\sniper rifle\sniper rifle", r"weapons\sniper rifle\effects\fire bullet"),
        (r"weapons\rocket launcher\rocket launcher", r"weapons\rocket launcher\effects\fire rocket"),
        (r"weapons\plasma pistol\plasma pistol", r"weapons\plasma pistol\effects\fire bolt"),
        (r"weapons\needler\needler", r"weapons\needler\effects\fire needle"),
        (r"weapons\flamethrower\flamethrower", r"weapons\flamethrower\effects\flame thrower jet"),
    ] {
        let g = lib.gun(gun).unwrap_or_else(|| panic!("{gun}"));
        assert_eq!(path(g.firing[0]), fire, "{gun}");
    }
    // The charged bolt fires its own effect (the plasma pistol's trigger 1).
    let pp = lib.gun(r"weapons\plasma pistol\plasma pistol").unwrap();
    assert!(pp.firing.iter().any(|f| path(*f) == r"weapons\plasma rifle\effects\charged bolt fire"), "{:?}", pp.firing.iter().map(|f| path(*f)).collect::<Vec<_>>());
    let proj = |p: &str| lib.projectile(p).unwrap_or_else(|| panic!("{p}: {:?}", lib.projectiles.keys().collect::<Vec<_>>()));
    let rocket = proj(r"weapons\rocket launcher\rocket");
    assert_eq!(path(rocket.detonation), r"weapons\rocket launcher\effects\rocket explosion");
    assert!(rocket.model && rocket.tracer.is_some(), "the rocket's model and exhaust contrail");
    let frag = proj(r"weapons\frag grenade\frag grenade");
    assert_eq!(path(frag.detonation), r"weapons\frag grenade\effects\explosion");
    assert!(frag.model && frag.light.is_some(), "the frag's model and light");
    let plasma = proj(r"weapons\plasma grenade\plasma grenade");
    assert_eq!(path(plasma.detonation), r"weapons\plasma grenade\effects\explosion");
    assert_eq!(path(plasma.detonation_started), r"weapons\plasma grenade\effects\detonation start");
    let needle = proj(r"weapons\needler\needle");
    assert_eq!(path(needle.detonation), r"weapons\needler\effects\needle detonate");
    assert_eq!(path(needle.super_detonation), r"weapons\needler\effects\explosion");
    let charged = proj(r"weapons\plasma rifle\charged bolt");
    assert_eq!(path(charged.detonation), r"weapons\plasma rifle\effects\detonate");
    // Every projectile's impacts on stone (2), but the flame's (it
    // vanishes: no effect).
    for p in [r"weapons\sniper rifle\sniper bullet", r"weapons\needler\needle", r"weapons\plasma pistol\bolt"] {
        assert!(proj(p).impacts.get(2).copied().flatten().is_some(), "{p}: a stone impact");
    }
    // Each explosion's sound decoded.
    for e in [rocket.detonation, frag.detonation, plasma.detonation] {
        let fx = &lib.effects[e.unwrap()];
        let sound = fx.events.iter().flat_map(|e| &e.parts).find_map(|p| match &p.kind {
            FxPartKind::Sound(s) => Some(s.clone()),
            _ => None,
        });
        let sound = sound.unwrap_or_else(|| panic!("{}: a sound", fx.path));
        assert!(!set.entries[&sound].waves.is_empty(), "{sound}");
    }
    // The frag's fireball (effects.md "Particle systems", the disc's
    // values): one type, 35 at once, explosion creation 0.012 / 0.08 WU at
    // 200 a second, fire then smoke plume.
    let med = lib.systems.iter().find(|s| s.path == r"weapons\frag grenade\effects\explosion med").expect("the frag's particle system");
    assert_eq!(med.types.len(), 1);
    let t = &med.types[0];
    assert_eq!((t.initial, t.creation), (35, 1));
    assert!((t.constants[0] - 0.012).abs() < 1e-4 && (t.constants[1] - 0.08).abs() < 1e-4 && (t.constants[2] - 200.0).abs() < 1e-2, "{:?}", t.constants);
    assert!(t.states.len() >= 2 && t.states[0].additive && !t.states[1].additive, "fire then smoke");
    assert!(t.states.iter().all(|s| s.material.is_some()));
    let parts: Vec<&FxPartKind> = lib.effects[frag.detonation.unwrap()].events.iter().flat_map(|e| e.parts.iter().map(|p| &p.kind)).collect();
    assert!(parts.iter().any(|p| matches!(p, FxPartKind::System(_))), "the frag's explosion makes its system");
    assert_eq!(lib.pickups[r"powerups\health pack"], r"halo_ce:sound\sfx\ui\pickup_health");
    assert_eq!(lib.pickups[r"powerups\over shield"], r"halo_ce:sound\sfx\ui\pickup_dbl_shield");
    assert_eq!(lib.pickups[r"weapons\frag grenade\frag grenade"], r"halo_ce:sound\sfx\weapons\weapon_pickup_ammo\grenade_ammo");
}

/// The objects drawn as shown items (`view::load::object_tags`): the
/// grenades and the projectiles with a model in flight, the guns and the
/// equipment on the ground, each with meshes.
#[test]
fn the_grenade_and_item_models_load() {
    let Some(map) = bloodgulch() else { return };
    use halo_ce::game::view::load::{item_key, looks, object_key};
    let l = looks(&map, false);
    let has = |key: String| l.held.iter().any(|h| h.key == key && !h.model.meshes.is_empty());
    for p in [r"weapons\frag grenade\frag grenade", r"weapons\plasma grenade\plasma grenade", r"weapons\rocket launcher\rocket", r"weapons\needler\needle"] {
        assert!(has(object_key(p)), "{p} in flight");
    }
    for i in [
        r"weapons\frag grenade\frag grenade",
        r"weapons\plasma grenade\plasma grenade",
        r"powerups\health pack",
        r"powerups\over shield",
        r"powerups\active camouflage",
        r"weapons\sniper rifle\sniper rifle",
        r"weapons\rocket launcher\rocket launcher",
    ] {
        assert!(has(item_key(i)), "{i} on the ground");
    }
}
