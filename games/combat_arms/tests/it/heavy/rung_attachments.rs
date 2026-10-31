//! The gun game's Combat Arms rung parts against the user's install
//! (docs/backlog.md "Rung attachments may not fit their gun"): for every
//! gun of the automatic ladder's pool (a Combat Arms weapon whose data
//! names a gun class), whether the modification file's `Weapon#` record
//! that gives it its accepted list (attachments.md Q13) is that weapon's,
//! and that every part a rung draws for it (`rungs::draw_parts`) is in
//! that list, can go on a weapon and sits in its own `Kinds` slot.
//!
//! Also: a fitted silencer and red dot drawn on the M4A1's view model
//! (`view_model::parts`).
//!
//! Needs the V1 key (`ATTRIBUTES.rez`; the view model the V2 key); skips
//! with a note without it. `-- --nocapture` prints the audit's numbers
//! (`AUDIT_ALL=1`: every record named otherwise; `AUDIT_SHOW=fmg,luger`:
//! the lists of the weapons whose names hold those).

use bevy::prelude::*;
use combat_arms::game::{
    attachments::{ATTACHMENTS, AttachParams, CaModTable, SLOTS},
    data::CaData,
    rungs::{self, RungOdds},
};
use chimera_sim::minigame::auto::Rng;

#[test]
fn every_pool_guns_record_is_its_own_and_every_drawn_part_fits() {
    let ca = CaData::install();
    let Some(attrs) = ca.attributes() else {
        eprintln!("skipping: the install's attribute files aren't readable (the V1 key)");
        return;
    };
    let (Some(file), Some(mods)) = (attrs.weapons.as_ref(), attrs.mods.as_ref()) else {
        eprintln!("skipping: no weapons or modification file");
        return;
    };
    let set = ca.weapon_set();
    let table: &CaModTable = ca.attachments();
    let (mut pool, mut by_position, mut by_name, mut unnamed, mut empty) = (0, 0, 0, 0, 0);
    let mut odd = Vec::new();
    // Per slot: guns whose record's option is 0, 1, 2 or unset; guns
    // whose list names parts for a slot the option closes; guns a rung
    // may draw a part for.
    let mut options = [[0usize; 4]; SLOTS];
    let mut closed = [0usize; SLOTS];
    let mut open = [0usize; SLOTS];
    for w in &set.weapons {
        if !w.id.starts_with("combat_arms:") || w.class.is_none() {
            continue;
        }
        let Some(params) = w.hook::<AttachParams>(ATTACHMENTS) else { continue };
        let Some(record) = ca.install_weapon(&w.id) else { continue };
        pool += 1;
        let index = file.weapons.iter().position(|x| std::ptr::eq(x, record)).unwrap();
        match mods.weapons.get(index) {
            Some(m) if m.name.eq_ignore_ascii_case(&record.name) => by_position += 1,
            Some(m) if m.name.is_empty() => unnamed += 1,
            Some(m) => {
                if let Some(j) = mods.weapons.iter().position(|x| x.name.eq_ignore_ascii_case(&record.name)) {
                    by_name += 1;
                    eprintln!("  NAME ELSEWHERE {index} \"{}\" record \"{}\"; record {j} has its name; lists equal: {}", record.name, m.name, mods.weapons[j].accepts == m.accepts);
                }
                if std::env::var("AUDIT_ALL").is_ok() || odd.len() < 20 {
                    odd.push(format!("{index}: weapon \"{}\", record \"{}\"", record.name, m.name));
                }
            }
            None => {}
        }
        if std::env::var("AUDIT_SHOW").is_ok_and(|v| v.split(',').any(|x| record.name.to_lowercase().contains(x))) {
            eprintln!("SHOW {index} {} ({}) record \"{}\" options {:?} defaults {:?}: {:?}", record.name, w.id, mods.weapons.get(index).map_or("", |m| m.name.as_str()), params.slot_options, params.defaults, params.accepts.iter().map(|a| format!("{}@{}", a.name(), a.variant())).collect::<Vec<_>>());
        }
        if params.accepts.is_empty() {
            empty += 1;
        }
        for slot in 0..SLOTS {
            let listed = params.accepts.iter().any(|a| table.find(a.name()).and_then(|i| table.get(i)).is_some_and(|r| r.slot() == Some(slot) && r.weapon_part()));
            let option = params.slot_options.get(slot).copied().unwrap_or(-1);
            let k = match option {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 3,
            };
            options[slot][k] += 1;
            if listed && matches!(option, 0 | 2) {
                closed[slot] += 1;
            }
            if !rungs::slot_candidates(table, &params, slot).is_empty() {
                open[slot] += 1;
            }
        }
        // Every part drawn over many seeds is listed, a weapon part, in
        // its own slot, at most one per slot.
        for seed in 0..50u64 {
            let mut rng = Rng::new(seed);
            let parts = rungs::draw_parts_with(table, &params, &RungOdds::default(), &mut rng);
            let mut slots = Vec::new();
            for p in &parts {
                assert!(params.variant_of(p).is_some(), "{}: {p} isn't in its list", w.id);
                let r = table.get(table.find(p).unwrap()).unwrap();
                assert!(r.weapon_part(), "{}: {p} can't go on a weapon", w.id);
                let slot = r.slot().unwrap();
                assert!(
                    !matches!(params.slot_options.get(slot), Some(0 | 2)),
                    "{}: {p} in slot {slot}, which its record closes",
                    w.id
                );
                assert!(!slots.contains(&slot), "{}: two parts for slot {slot}", w.id);
                slots.push(slot);
            }
        }
    }
    let mismatched = pool - by_position - unnamed;
    eprintln!(
        "pool {pool}: record named as the weapon {by_position}, unnamed {unnamed}, named otherwise {mismatched} \
         (of those, a record of the weapon's name elsewhere: {by_name}); empty accepted lists {empty}; \
         weapons file {} records, modification file {}",
        file.weapons.len(),
        mods.weapons.len()
    );
    for slot in 0..SLOTS {
        eprintln!(
            "slot {slot}: option 0/1/2/unset {:?}; listed parts the option closes {}; drawable {}",
            options[slot], closed[slot], open[slot]
        );
    }
    for o in &odd {
        eprintln!("  {o}");
    }
    assert!(pool > 100, "the install's guns are in the pool ({pool})");
    // Q13 holds: a record named otherwise is a spelling of the weapon's
    // name or a shared list, with two swapped exceptions (the `loaded`
    // R870 and AT-22), which the game links by position too.
    assert!(by_name <= 2, "{by_name} weapons have a record of their own name elsewhere");
}


/// The install's mount (skips with a note without it).
fn mount() -> Option<chimera_core::mount::Mount> {
    use chimera_core::mount::config::LocalConfig;
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(combat_arms::game::GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((combat_arms::game::INFO.open)(&install).unwrap())
}

/// A silencer and a red dot fitted to the M4A1 (attachments.md "View
/// model and world model"): its view-model key names them
/// (`parts::view_key`), and that view model is the gun's with each part's
/// meshes added, skinned rigidly to its socket's node (`Flash`, `Breach`)
/// and drawn near that socket, textured; a magazine adds none.
#[test]
fn a_fitted_silencer_and_sight_draw_on_the_view_model() {
    use combat_arms::game::{
        attachments::{CaAttachSlots, CaFitted, Stock, mods_of},
        view_model::{on_demand, parts},
    };
    const M4: &str = "combat_arms:m4a1";
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.mods.as_ref()).is_none() {
        eprintln!("skipping: the install's modification file isn't readable (the V1 key)");
        return;
    }
    let table = ca.attachments();
    let params = ca.weapon_set().get(M4).unwrap().hook::<AttachParams>(ATTACHMENTS).unwrap();
    let slots = CaAttachSlots {
        params: params.clone(),
        stock: Stock::default(),
    };
    let fitted = |names: &[&str]| {
        let mut f = CaFitted::default();
        for n in names {
            let id = table.find(n).unwrap();
            f.slots[table.get(id).unwrap().slot().unwrap()] = Some(id);
        }
        mods_of(table, &slots, &f)
    };
    let key = parts::view_key(M4, Some(&fitted(&["Silencer_A_AR", "DotSight_A_AR", "Magazine_A_AR"])));
    assert_eq!(key, format!("{M4}+Silencer_A_AR+DotSight_A_AR"));
    assert_eq!(parts::view_key(M4, Some(&fitted(&["Magazine_A_AR"]))), M4);
    let bare = match on_demand::load_weapon(M4, &file) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("skipping: the M4A1's view model doesn't load ({e}; the V2 key)");
            return;
        }
    };
    let with = on_demand::load_weapon(&key, &file).unwrap();
    for w in &with.warnings {
        eprintln!("warning: {w}");
    }
    assert!(with.warnings.iter().all(|w| !w.contains("part")), "{:?}", with.warnings);
    assert_eq!(with.model.key, key);
    let (b, v) = (&bare.model, &with.model);
    let added = &v.model.meshes[b.model.meshes.len()..];
    assert!(added.len() >= 2, "the parts' meshes are added ({})", added.len());
    let socket = |name: &str| v.attachments.iter().find(|a| a.name.eq_ignore_ascii_case(name)).unwrap();
    let globals = combat_arms::game::view_model::hands::globals(&v.bones);
    let at = |name: &str| {
        let s = socket(name);
        (v.root.to_matrix() * globals[s.bone] * s.local.to_matrix()).transform_point3(Vec3::ZERO)
    };
    for name in ["Flash", "Breach"] {
        let s = socket(name);
        let on: Vec<_> = added.iter().filter(|m| m.joints.first().is_some_and(|j| j[0] as usize == s.bone)).collect();
        assert!(!on.is_empty(), "a part on the {name} socket's node");
        // Its middle lies near the socket (metres).
        let mid = on.iter().flat_map(|m| m.positions.iter()).fold(Vec3::ZERO, |a, p| a + Vec3::from(*p))
            / on.iter().map(|m| m.positions.len()).sum::<usize>() as f32;
        let d = mid.distance(at(name));
        eprintln!("{name}: {} meshes, middle {d:.3} m from the socket", on.len());
        assert!(d < 0.6, "{name}'s part {d} m from its socket");
        assert!(on.iter().all(|m| m.joint_weights.iter().all(|w| w[0] == 1.0)));
    }
    // The silencer reaches ahead of the muzzle (away from the gun's
    // middle, in the `Flash` socket's frame), its back end at it: the
    // part in the socket's own frame, its model in its first animation's
    // pose, no turn of ours (attachments.md Corrections (2026-10-30) 1, 5).
    let fl = socket("Flash");
    let inv = (v.root.to_matrix() * globals[fl.bone] * fl.local.to_matrix()).inverse();
    let gun: Vec<Vec3> = b.model.meshes.iter().flat_map(|m| m.positions.iter()).map(|p| inv.transform_point3(Vec3::from(*p))).collect();
    let ahead = -(gun.iter().fold(Vec3::ZERO, |a, p| a + *p) / gun.len() as f32).normalize();
    let silencer = added.iter().find(|m| m.material.contains("silencer")).expect("the silencer's mesh");
    let along: Vec<f32> = silencer.positions.iter().map(|p| inv.transform_point3(Vec3::from(*p)).dot(ahead)).collect();
    let (lo, hi) = along.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    eprintln!("silencer from {lo:.2} to {hi:.2} units ahead of the muzzle");
    assert!(lo < 0.3 && hi > 0.5, "the silencer from {lo} to {hi} units ahead of the muzzle");
    assert!(added.iter().any(|m| m.texture.is_some()), "the parts are textured");
    assert!(with.textures.len() > bare.textures.len());
}


/// The same parts on the M4A1's third-person (held) model
/// (`characters::models::load_held_weapon` with a `+Part` key): its
/// meshes with each part's added, the silencer reaching ahead of the
/// gun's front end, the sight above the gun.
#[test]
fn fitted_parts_draw_on_the_held_model() {
    use combat_arms::game::characters::models::load_held_weapon;
    const M4: &str = "combat_arms:m4a1";
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    if CaData::install().attributes().and_then(|a| a.mods.as_ref()).is_none() {
        eprintln!("skipping: the install's modification file isn't readable (the V1 key)");
        return;
    }
    let (bare, _) = match load_held_weapon(M4, &file) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("skipping: the M4A1's held model doesn't load ({e}; the V2 key)");
            return;
        }
    };
    let (with, _) = load_held_weapon(&format!("{M4}+Silencer_A_AR+DotSight_A_AR"), &file).unwrap();
    let added = &with.model.meshes[bare.model.meshes.len()..];
    assert!(added.len() >= 2, "the parts' meshes are added ({})", added.len());
    let gun: Vec<Vec3> = bare.model.meshes.iter().flat_map(|m| m.positions.iter()).map(|p| Vec3::from(*p)).collect();
    let bounds = |pts: &mut dyn Iterator<Item = Vec3>| pts.fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(p), b.max(p)));
    let (glo, ghi) = bounds(&mut gun.iter().copied());
    // The gun's long axis.
    let size = ghi - glo;
    let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
    for m in added {
        let (lo, hi) = bounds(&mut m.positions.iter().map(|p| Vec3::from(*p)));
        eprintln!("held part {}: {lo:.1}..{hi:.1} (gun {glo:.1}..{ghi:.1}, long axis {axis})", m.material);
    }
    let silencer = added.iter().find(|m| m.material.to_lowercase().contains("silencer")).expect("the silencer's mesh");
    let (slo, shi) = bounds(&mut silencer.positions.iter().map(|p| Vec3::from(*p)));
    // It sticks out past one end of the gun along its long axis.
    assert!(shi[axis] > ghi[axis] || slo[axis] < glo[axis], "the silencer reaches past the gun's end");
}


