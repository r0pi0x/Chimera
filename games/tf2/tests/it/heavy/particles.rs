//! TF2's particle systems as our loader reads them, run in
//! `chimera_present::render::psys` from the user's install (passes with a
//! note without it). The flash test is the check that the remaps by
//! creation time load and run (a muzzle flash once came out four times its
//! size: tools/tf2ref, docs/plans/active/tf2-parity.md). The `dump_*`
//! tests print a system for looking into one (`PROBE_SYS=<name>`,
//! `PROBE_PCF=particles/<file>.pcf`; `--nocapture`).
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
    run(&map, &name, &[0.01, 0.03, 0.06], |t, i| {
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
