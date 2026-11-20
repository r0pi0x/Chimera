//! TF2's particle systems as our loader reads them, run in
//! `chimera_present::render::psys` from the user's install (passes with a
//! note without it). The flash test is the check that the remaps by
//! creation time load and run (a muzzle flash once came out four times its
//! size: tools/tf2ref, docs/plans/active/tf2-parity.md). The `dump_*`
//! tests print a system for looking into one (`PROBE_SYS=<name>`,
//! `PROBE_PCF=particles/<file>.pcf`, `PROBE_ALPHA=1` for each
//! particle's place, age, life, alpha and radius; `--nocapture`).
use chimera_present::render::psys::{ControlPoint, Instance, MaterialSheets};
use chimera_world::map::{MapData, ParticleRng, psys::SystemDef};
use source_engine::content::pcf::{Value, read_dmx};

fn run(map: &MapData, name: &str, times: &[f32], mut each: impl FnMut(f32, &Instance)) {
    let systems = &map.particle_systems;
    let sheets = MaterialSheets(Some(&map.particles));
    let def = systems.find(name).unwrap_or_else(|| panic!("{name}: not loaded"));
    let mut inst = Instance::new(&systems.defs, def);
    let cps = vec![ControlPoint { rotation: bevy::math::Quat::IDENTITY, light: bevy::math::Vec3::ONE, ..Default::default() }];
    let mut rng = ParticleRng::new(1);
    let mut t = 0.0;
    for &target in times {
        while t < target - 1e-6 {
            let h = (target - t).min(0.002);
            inst.step(&systems.defs, &cps, h, &mut rng, &sheets);
            t += h;
        }
        each(t, &inst);
    }
}

fn show(i: &Instance, defs: &[SystemDef], map: &MapData, depth: usize) {
    let d = &defs[i.def];
    let r = i.own_radii(defs);
    let mat = d.material.map(|m| &map.particles.materials[m]);
    eprintln!(
        "{:indent$}{} n={} rmax={:.1} mat={:?} blend={:?} ob={:?} render={:?}",
        "",
        d.name,
        r.len(),
        r.iter().cloned().fold(0.0f32, f32::max),
        mat.map(|m| &m.name),
        mat.map(|m| m.blend),
        mat.map(|m| m.overbright),
        d.renderer,
        indent = depth * 2
    );
    if std::env::var("PROBE_ALPHA").is_ok() {
        for ((at, age, life, alpha), r) in i.own_details(defs).iter().zip(&r) {
            eprintln!("{:indent$}  at {at:.1} age {age:.3} life {life:.3} alpha {alpha:.2} r {r:.2}", "", indent = depth * 2);
        }
    }
    for c in i.child_instances() {
        show(c, defs, map, depth + 1);
    }
}

/// The Scattergun's (and Shotgun's, SMG's) flash child: radius 8 to 5,
/// then 3 to 0.5 by creation time (specs/tf2/particles.md 8.1), never the
/// file's base radius 32.
#[test]
fn the_flash_child_keeps_its_remapped_size() {
    let Some(map) = super::looks::dressed() else { return };
    let defs = &map.particle_systems.defs;
    run(&map, "muzzle_scattergun_flash", &[0.004, 0.02, 0.04, 0.08], |t, i| {
        let r = i.own_radii(defs);
        let max = r.iter().cloned().fold(0.0f32, f32::max);
        assert!(!r.is_empty() || t > 0.06, "{t}: none alive");
        assert!(max <= 8.0 + 1e-3, "{t}: radius {max}");
    });
}

#[test]
fn dump_a_system_over_time() {
    let Some(map) = super::looks::dressed() else { return };
    let name = std::env::var("PROBE_SYS").unwrap_or("muzzle_smg".into());
    let times: Vec<f32> = std::env::var("PROBE_T").map(|s| s.split(",").filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![0.01, 0.03, 0.06]);
    run(&map, &name, &times, |t, i| {
        eprintln!("--- t={t}");
        show(i, &map.particle_systems.defs, &map, 0);
    });
}

#[test]
fn dump_a_system_as_loaded_and_in_its_file() {
    let Some(map) = super::looks::dressed() else { return };
    let systems = &map.particle_systems;
    let name = std::env::var("PROBE_SYS").unwrap_or("muzzle_scattergun_flash".into()).to_lowercase();
    if let Some(i) = systems.find(&name) {
        eprintln!("{:#?}", systems.defs[i]);
        for (c, delay) in &systems.defs[i].children {
            eprintln!("child {} after {delay}", systems.defs[*c].name);
        }
    }
    for w in map.warnings.iter().filter(|w| w.contains("particle")) {
        eprintln!("warning: {w}");
    }
    let Some(mount) = super::maps::install_mount() else { return };
    let file = std::env::var("PROBE_PCF").unwrap_or("particles/muzzle_flash.pcf".into());
    let Ok(elements) = read_dmx(&mount.read(&file).unwrap()) else { return };
    let Some(sys) = elements.iter().find(|e| e.kind == "DmeParticleSystemDefinition" && e.name.to_lowercase() == name) else {
        return;
    };
    for (k, v) in &sys.attributes {
        let Value::Array(items) = v else {
            eprintln!("{k} = {v:?}");
            continue;
        };
        eprintln!("{k}:");
        for it in items {
            if let Value::Element(Some(i)) = it {
                let e = &elements[*i];
                eprintln!("  [{}] {}", e.kind, e.name);
                for (k2, v2) in &e.attributes {
                    eprintln!("      {k2} = {v2:?}");
                }
            }
        }
    }
}

/// specs/tf2/particles.md 5.1 against the install's `ExplosionCore_Wall`
/// stepped at 60 a second: the flying embers draw thin rays (a trail and a
/// head sprite each, the strips a few units wide), the core flash broad
/// streaks (tens of units). The flash-up (3000/s for 0.01 s) makes its 30
/// particles in its first emitting step (particles.md 9.4) and draws them
/// in a blast (10.1): strips 32 long (its minimum), at most as wide, about
/// a dozen at full alpha; skipped ahead 0.04 s (an info_particle_system's
/// late start, 10.10) it makes none.
#[test]
fn the_blast_draws_thin_ember_rays_broad_core_streaks_and_flashups_thirty() {
    use chimera_world::map::particles::Shape;
    let Some(map) = super::looks::dressed() else { return };
    let systems = &map.particle_systems;
    let defs = &systems.defs;
    let embers = &defs[systems.find("Explosion_FlyingEmbers").expect("loaded")];
    assert!(
        matches!(embers.renderer, chimera_world::map::psys::Render::Trail { .. })
            != matches!(embers.renderer, chimera_world::map::psys::Render::Sprites { .. })
            && embers.renderers().count() == 2,
        "a trail and a sprite: {:?} {:?}",
        embers.renderer,
        embers.more_renderers
    );
    let sheets = MaterialSheets(Some(&map.particles));
    let cps = vec![ControlPoint { rotation: bevy::math::Quat::IDENTITY, light: bevy::math::Vec3::ONE, ..Default::default() }];
    let mut rng = ParticleRng::new(7);
    let width = |name: &str, steps: usize, rng: &mut ParticleRng| -> (usize, f32, usize) {
        let mut i = Instance::new(defs, systems.find(name).unwrap());
        for _ in 0..steps {
            i.step(defs, &cps, 1.0 / 60.0, rng, &sheets);
        }
        let mut out = Vec::new();
        i.draw(defs, &|v| v, 1.0, bevy::math::Vec3::new(0.0, 0.0, 10_000.0), &sheets, &mut out);
        let strips: Vec<f32> = out
            .iter()
            .filter_map(|q| match q.shape {
                Shape::Quad { corners, .. } => Some(corners[0].distance(corners[3])),
                _ => None,
            })
            .collect();
        (strips.len(), strips.iter().cloned().fold(0.0, f32::max), i.count())
    };
    let (n, widest, _) = width("Explosion_FlyingEmbers", 2, &mut rng);
    assert!(n > 4 && widest <= 3.0 + 1e-3, "{n} rays, widest {widest}");
    let (n, widest, _) = width("Explosion_CoreFlash", 2, &mut rng);
    assert!(n > 4 && widest > 10.0, "{n} streaks, widest {widest}");
    let (_, _, made) = width("Explosion_Flashup", 1, &mut rng);
    assert_eq!(made, 0, "nothing in its creation frame");
    let mut i = Instance::new(defs, systems.find("Explosion_Flashup").unwrap());
    i.step(defs, &cps, 1.0 / 60.0, &mut rng, &sheets);
    i.step(defs, &cps, 1.0 / 60.0, &mut rng, &sheets);
    // 30 made; those whose life U(0, 0.2) is shorter than their age by
    // the step's end are gone.
    let seen = i.count();
    assert!((20..=30).contains(&seen), "the flash-up's first emitting step: {seen}");
    let mut out = Vec::new();
    i.draw(defs, &|v| v, 1.0, bevy::math::Vec3::new(0.0, 0.0, 10_000.0), &sheets, &mut out);
    let full = out.iter().filter(|q| q.alpha == 1.0).count();
    assert!((4..=24).contains(&full), "{full} of {} at full alpha", out.len());
    for q in &out {
        let Shape::Quad { corners, .. } = q.shape else { panic!("{:?}", q.shape) };
        let (long, wide) = (corners[0].distance(corners[1]), corners[0].distance(corners[3]));
        assert!((long - 32.0).abs() < 0.1 && wide <= long + 1e-3, "{long} x {wide}");
    }
    let mut skipped = Instance::new(defs, systems.find("Explosion_Flashup").unwrap());
    skipped.skip_to(defs, 0.04);
    for _ in 0..20 {
        skipped.step(defs, &cps, 1.0 / 60.0, &mut rng, &sheets);
        assert_eq!(skipped.count(), 0, "skipped: none made");
    }
}
