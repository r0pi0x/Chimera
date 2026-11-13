//! Every Combat Arms gun's drawable parts against its model (docs/backlog.md
//! "Silencers detached from skinned guns"): for each gun of the install
//! and each part it can be drawn with (a rung's candidates,
//! `rungs::slot_candidates`, and its drawn default parts) that has a model
//! (a socket slot's part with an `AttachModel`c / `AttachHHModel`c for
//! the gun's variant index), the part on the gun's view model
//! (`on_demand::load_weapon` with a `+Part` key, in its idle pose) and on
//! its held model (`characters::models::load_held_weapon`), and whether
//! it touches the gun's own meshes (some vertex within `OFF` of the gun's
//! length of one of the gun's; attachments.md "View model and world
//! model": a part sits at a socket of the gun). Guns whose parts the
//! install's own models put off (`INSTALL_OFF`) are listed, not failed.
//!
//! Needs the install's keys; skips with a note without them.
//! `-- --nocapture` prints the census; `SWEEP_ONLY=glock_23,l96a1` limits
//! it to weapons whose ids hold those (and asserts nothing);
//! `SWEEP_SHOW=1` prints each gun's models, part offsets and parts.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::prelude::*;
use combat_arms::game::{
    attachments::{ATTACHMENTS, AttachParams, SIGHT, SOCKET_SLOTS},
    characters::models::load_held_weapon,
    data::CaData,
    rungs,
    view_model::on_demand,
};

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

/// A part counts as off its gun when no vertex of it comes within this
/// fraction of the gun's length of one of the gun's own (a silencer at
/// the muzzle or a sight on the rail touches it; the stock M4A1's held
/// silencer comes within 0.07, the camouflaged MINIMI's red dot on its
/// mount 0.13).
const OFF: f32 = 0.15;

/// Gun and part pairs the install's own data puts off the gun (first or
/// third person, the weapon id), which the game reads as they are
/// (attachments.md "View model and world model"): the `flash` socket of
/// the Azazel Heartseeker's and the two Groza skins' models lies 23 units
/// ahead of the muzzle; the SIG50 Lava's and the Devil Soul P90's `Flash`
/// sockets sit past their barrels' ends.
const INSTALL_OFF: &[(&str, &str)] = &[
    ("view", "combat_arms:azazel_heartseeker"),
    ("view", "combat_arms:sk_groza"),
    ("view", "combat_arms:groza_tropical"),
    ("view", "combat_arms:sig50_lava"),
    ("view", "combat_arms:devil_soul_p90"),
    ("held", "combat_arms:devil_soul_p90"),
];

/// The gun's own vertices (on the view model: not the arms' and hands',
/// whose bones are `Bip01 ...`), in cells of `OFF` of its length (the
/// longest side of their box).
struct Gun {
    cell: f32,
    grid: HashMap<(i32, i32, i32), Vec<Vec3>>,
    points: Vec<Vec3>,
    length: f32,
}

impl Gun {
    fn new(points: Vec<Vec3>) -> Option<Self> {
        let (lo, hi) = points.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        let length = (hi - lo).max_element();
        if points.is_empty() || length <= 0.0 {
            return None;
        }
        let cell = length * OFF;
        let mut grid: HashMap<_, Vec<Vec3>> = HashMap::new();
        for p in &points {
            grid.entry(Self::key(*p, cell)).or_default().push(*p);
        }
        Some(Gun { cell, grid, points, length })
    }

    fn key(p: Vec3, cell: f32) -> (i32, i32, i32) {
        let k = (p / cell).floor();
        (k.x as i32, k.y as i32, k.z as i32)
    }

    /// Whether some vertex of `part` lies within `OFF` of the gun's length
    /// of one of the gun's.
    fn touches(&self, part: &[Vec3]) -> bool {
        part.iter().any(|p| {
            let (x, y, z) = Self::key(*p, self.cell);
            (-1..=1).any(|dx| {
                (-1..=1).any(|dy| {
                    (-1..=1).any(|dz| {
                        self.grid
                            .get(&(x + dx, y + dy, z + dz))
                            .is_some_and(|g| g.iter().any(|q| q.distance(*p) <= self.cell))
                    })
                })
            })
        })
    }

    /// The part's nearest distance to the gun, as a fraction of its length.
    fn gap(&self, part: &[Vec3]) -> f32 {
        let step = (self.points.len() / 20_000).max(1);
        let d = part
            .iter()
            .map(|p| self.points.iter().step_by(step).map(|g| g.distance_squared(*p)).fold(f32::MAX, f32::min))
            .fold(f32::MAX, f32::min);
        d.sqrt() / self.length
    }
}

/// Per bone, from the view model's reference pose to its idle pose (its
/// first `idle` sequence at its start; none: the reference pose), in the
/// eye's space: what is on screen.
fn idle_skin(v: &chimera_world::map::MapViewModel) -> Vec<Mat4> {
    let reference = combat_arms::game::view_model::hands::globals(&v.bones);
    let Some(set) = v.animations.as_ref() else { return vec![Mat4::IDENTITY; v.bones.len()] };
    let Some(s) = set.sequences.iter().position(|q| q.name.to_lowercase().starts_with("idle")) else {
        return vec![Mat4::IDENTITY; v.bones.len()];
    };
    let mut pose = set.defaults.clone();
    set.sequence_pose(s, 0.0, &set.default_params(), &mut pose);
    let mut posed: Vec<Mat4> = Vec::with_capacity(v.bones.len());
    for (i, b) in v.bones.iter().enumerate() {
        let (r, t) = pose.get(i).copied().unwrap_or((b.rotation, b.position));
        let local = Mat4::from_rotation_translation(r, t);
        let g = b.parent.and_then(|p| posed.get(p)).map_or(local, |p| *p * local);
        posed.push(g);
    }
    let root = v.root.to_matrix();
    posed.iter().zip(&reference).map(|(p, r)| root * *p * r.inverse() * root.inverse()).collect()
}

/// `meshes`' vertices on bones `keep` takes, each moved by its bone's
/// `skin` matrix.
fn posed_points<'a>(meshes: impl Iterator<Item = &'a chimera_world::map::MapMesh>, skin: &[Mat4], keep: impl Fn(u16) -> bool) -> Vec<Vec3> {
    meshes
        .flat_map(|m| m.positions.iter().zip(&m.joints))
        .filter(|(_, j)| keep(j[0]))
        .map(|(p, j)| skin.get(j[0] as usize).copied().unwrap_or(Mat4::IDENTITY).transform_point3(Vec3::from(*p)))
        .collect()
}

fn points<'a>(meshes: impl Iterator<Item = &'a chimera_world::map::MapMesh>, keep: impl Fn(u16) -> bool) -> Vec<Vec3> {
    meshes
        .flat_map(|m| {
            let joints = m.joints.iter().map(|j| j[0]).chain(std::iter::repeat(0));
            m.positions.iter().zip(joints).filter(|(_, j)| m.joints.is_empty() || keep(*j)).map(|(p, _)| Vec3::from(*p))
        })
        .collect()
}

#[test]
fn every_guns_parts_sit_on_its_model() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.mods.as_ref()).is_none() {
        eprintln!("skipping: the install's modification file isn't readable (the V1 key)");
        return;
    }
    let only: Vec<String> = std::env::var("SWEEP_ONLY")
        .map(|v| v.split(',').map(str::to_lowercase).collect())
        .unwrap_or_default();
    let table = ca.attachments();
    let set = ca.weapon_set();
    let (mut checked, mut guns) = (0usize, 0usize);
    let mut off: Vec<String> = Vec::new();
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    // One check per distinct (model, offsets, part, variant): skins of a
    // gun with the same model and record numbers give the same answer.
    let mut seen_view = HashSet::new();
    let mut seen_held = HashSet::new();
    for w in &set.weapons {
        if !w.id.starts_with("combat_arms:") {
            continue;
        }
        if !only.is_empty() && !only.iter().any(|o| w.id.contains(o.as_str())) {
            continue;
        }
        let Some(params) = w.hook::<AttachParams>(ATTACHMENTS) else { continue };
        // The parts it can be drawn with: those a rung may draw
        // (`rungs::slot_candidates`: listed, in a slot its record leaves
        // open) and its drawn default parts.
        let mut parts = Vec::new();
        for slot in 0..SOCKET_SLOTS {
            let mut names = rungs::slot_candidates(table, &params, slot);
            if let Some(d) = params.defaults.get(slot)
                && (slot != SIGHT || params.default_sight_visible)
            {
                names.push(d);
            }
            for name in names {
                let Some(r) = table.find(name).and_then(|i| table.get(i)) else { continue };
                let Some(c) = params.variant_of(&r.name).and_then(|c| usize::try_from(c).ok()).filter(|c| *c < 10) else { continue };
                if r.weapon_part() && r.slot() == Some(slot) {
                    parts.push((r, slot, c));
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        guns += 1;
        let offs = format!("{:?}", params.part_offsets);
        if std::env::var("SWEEP_SHOW").is_ok() {
            eprintln!("GUN {} view {:?} world {:?} offsets {offs} parts {:?}", w.id, w.models.view, w.models.world, parts.iter().map(|(r, _, c)| format!("{}@{c}", r.name)).collect::<Vec<_>>());
        }
        // First person.
        if let Some(view) = w.models.view.as_deref() {
            let mut bare = None;
            for (r, slot, c) in &parts {
                let Some((model, _)) = r.view_model(*c) else { continue };
                if !seen_view.insert((view.to_lowercase(), offs.clone(), r.socket.clone(), model.to_lowercase(), *slot)) {
                    continue;
                }
                let bare = match bare.get_or_insert_with(|| on_demand::load_weapon(&ca, &w.id, &file)) {
                    Ok(b) => b,
                    Err(e) => {
                        *errors.entry(format!("view model: {}", e.split(':').next_back().unwrap_or(e).trim())).or_default() += 1;
                        break;
                    }
                };
                let key = format!("{}+{}", w.id, r.name);
                let with = match on_demand::load_weapon(&ca, &key, &file) {
                    Ok(v) => v,
                    Err(e) => {
                        *errors.entry(format!("view part: {e}")).or_default() += 1;
                        continue;
                    }
                };
                for e in &with.warnings {
                    *errors.entry(e.clone()).or_default() += 1;
                }
                let n = bare.model.model.meshes.len();
                let bones = &bare.model.bones;
                let own = |j: u16| bones.get(j as usize).is_none_or(|b| !b.name.starts_with("Bip01"));
                let skin = idle_skin(&with.model);
                let Some(gun) = Gun::new(posed_points(bare.model.model.meshes.iter(), &skin, own)) else { continue };
                let part = posed_points(with.model.model.meshes[n..].iter(), &skin, |_| true);
                if part.is_empty() {
                    continue;
                }
                checked += 1;
                if !gun.touches(&part) {
                    let g = gun.gap(&part);
                    off.push(format!("view {} ({view}) + {} [{c}] at `{}`: {g:.2} of the gun's length off", w.id, r.name, r.socket.as_deref().unwrap_or("")));
                }
            }
        }
        // Third person.
        if let Some(world) = w.models.world.as_deref() {
            let mut bare = None;
            for (r, slot, c) in &parts {
                let Some((model, _)) = r.held_model(*c) else { continue };
                if !seen_held.insert((world.to_lowercase(), offs.clone(), r.hh_socket.clone(), model.to_lowercase(), *slot)) {
                    continue;
                }
                let bare = match bare.get_or_insert_with(|| load_held_weapon(&ca, &w.id, &file).map(|h| h.model)) {
                    Ok(b) => b,
                    Err(e) => {
                        *errors.entry(format!("held model: {}", e.split(':').next_back().unwrap_or(e).trim())).or_default() += 1;
                        break;
                    }
                };
                let key = format!("{}+{}", w.id, r.name);
                let with = match load_held_weapon(&ca, &key, &file) {
                    Ok(h) => h.model,
                    Err(e) => {
                        *errors.entry(format!("held part: {e}")).or_default() += 1;
                        continue;
                    }
                };
                let n = bare.model.meshes.len();
                if with.model.meshes.len() <= n {
                    *errors.entry(format!("held part not added: {}", r.name)).or_default() += 1;
                    continue;
                }
                let Some(gun) = Gun::new(points(bare.model.meshes.iter(), |_| true)) else { continue };
                let part = points(with.model.meshes[n..].iter(), |_| true);
                checked += 1;
                if !gun.touches(&part) {
                    let g = gun.gap(&part);
                    off.push(format!("held {} ({world}) + {} [{c}] at `{}`: {g:.2} of the gun's length off", w.id, r.name, r.hh_socket.as_deref().unwrap_or("")));
                }
            }
        }
    }
    eprintln!("{guns} guns with drawn parts, {checked} gun-part models checked, {} off", off.len());
    for o in &off {
        eprintln!("  OFF {o}");
    }
    for (e, n) in &errors {
        eprintln!("  {n} x {e}");
    }
    if !only.is_empty() {
        return;
    }
    assert!(guns > 1000, "the install's guns are swept ({guns})");
    let unlisted: Vec<&String> = off
        .iter()
        .filter(|o| !INSTALL_OFF.iter().any(|(kind, id)| o.starts_with(&format!("{kind} {id} "))))
        .collect();
    assert!(unlisted.is_empty(), "{} gun-part models sit off their gun: {unlisted:#?}", unlisted.len());
}
