//! Halo's guns in hand from the disc (`game::view`, specs/halo_ce/models.md
//! "Measured facts", animations.md "First-person graphs", "The graph and
//! its models"; weapons.md 16): the first-person models and the arms with
//! the spec's node, vertex and marker counts; each first-person graph's
//! node count and every filled slot's frame, key and loop numbers; every
//! node of the arms and of each gun in its graph by name; the four view
//! models and held models built from bloodgulch, each gun's draw, reload,
//! shell and melee animations as long as its document's timers; and the
//! assault rifle's idle placing `frame gun` where the spec's sanity check
//! puts it in front of the camera; another game's map dressed with the
//! Spartan and the four guns. Passes with a note without an install.

use blam_formats::{
    animation::{AnimationGraph, FirstPersonSlot as S},
    cache::{MapFile, class},
    model::Model,
    weapon::Weapon,
};
use chimera_world::map::anim::BonePose;
use halo_ce::game::{
    body::{load::DONOR_MAP, skeleton::globals},
    maps,
    view::{
        load::{self, EJECTION, MUZZLE},
        machine::{numbers, sequence},
    },
    weapons::{ASSAULT_RIFLE, PISTOL, PLASMA_RIFLE, SHOTGUN, gun},
};

use super::maps::mount;

fn bloodgulch() -> Option<MapFile> {
    let bytes = mount()?.read(&maps::map_path(DONOR_MAP)).unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

fn vertices(m: &Model) -> usize {
    m.highest_detail_parts().iter().map(|p| p.part.vertices.len()).sum()
}

fn fp(file: &MapFile, tag: &str) -> (Model, AnimationGraph) {
    let w = Weapon::read(file, file.find(class::WEAPON, tag).unwrap()).unwrap();
    let m = Model::read(file, file.tag(w.own.first_person_model.id).unwrap()).unwrap();
    let g = AnimationGraph::read(file, file.tag(w.own.first_person_animations.id).unwrap()).unwrap();
    (m, g)
}

#[test]
fn first_person_models_and_graphs_match_the_spec() {
    let Some(file) = bloodgulch() else { return };
    let arms = load::arms(&file).unwrap();
    assert_eq!(arms.nodes.len(), 37, "the arms: 37 nodes");
    assert_eq!(vertices(&arms), 1440);
    assert!(arms.markers.is_empty());
    // (gun, model nodes, vertices, marker names, graph nodes).
    let markers = |m: &Model| {
        m.markers
            .iter()
            .map(|m| (m.name.clone(), m.instances.len()))
            .collect::<Vec<_>>()
    };
    for (id, nodes, verts, names, graph_nodes) in [
        (
            ASSAULT_RIFLE,
            5,
            Some(1713),
            vec![
                ("flashlight", 1),
                ("left hand cyborg", 1),
                ("left hand marine", 1),
                ("primary ejection", 1),
                ("primary trigger", 1),
                ("smoke", 1),
            ],
            42,
        ),
        (
            PISTOL,
            7,
            Some(1728),
            vec![
                ("flashlight", 1),
                ("laser", 1),
                ("left hand cyborg", 1),
                ("left hand marine", 1),
                ("primary ejection", 1),
                ("primary trigger", 1),
            ],
            44,
        ),
        (
            PLASMA_RIFLE,
            5,
            Some(1957),
            vec![
                ("flashlight", 1),
                ("ground point", 1),
                ("heat flare", 1),
                ("heat flare1", 1),
                ("left hand", 1),
                ("primary trigger", 1),
                ("primary trigger1", 1),
                ("secondary trigger", 1),
                ("vent", 8),
            ],
            42,
        ),
        // The spec gives no shotgun first-person model numbers.
        (SHOTGUN, 4, None, vec![], 41),
    ] {
        let (model, graph) = fp(&file, &gun(id).tag);
        if verts.is_some() {
            assert_eq!(model.nodes.len(), nodes, "{id}: nodes");
            let names: Vec<(String, usize)> = names.iter().map(|(n, k)| (n.to_string(), *k)).collect();
            assert_eq!(markers(&model), names, "{id}: markers");
        }
        if let Some(v) = verts {
            assert_eq!(vertices(&model), v, "{id}: vertices");
        }
        assert_eq!(graph.nodes.len(), graph_nodes, "{id}: graph nodes");
        assert_eq!(
            graph.nodes.len(),
            arms.nodes.len() + model.nodes.len(),
            "{id}: the arms' and the gun's nodes"
        );
        assert!(
            graph.match_model(&arms).is_complete(),
            "{id}: every arm node in the graph"
        );
        assert!(
            graph.match_model(&model).is_complete(),
            "{id}: every gun node in the graph"
        );
    }
}

/// A slot, its frames, key frame and loop frame.
type Slot = (S, i16, i16, i16);

#[test]
fn first_person_slots_have_the_specs_frames() {
    let Some(file) = bloodgulch() else { return };
    // animations.md "First-person graphs": (slot, frames, key, loop).
    let table: [(&str, Vec<Slot>); 4] = [
        (
            ASSAULT_RIFLE,
            vec![
                (S::Ready, 29, 0, 0),
                (S::PutAway, 11, 0, 0),
                (S::ReloadEmpty, 87, 0, 0),
                (S::ReloadFull, 87, 0, 0),
                (S::Melee, 37, 3, 0),
                (S::ThrowGrenade, 37, 0, 0),
                (S::Idle, 80, 0, 0),
                (S::Posing, 118, 0, 0),
                (S::Moving, 26, 0, 0),
                (S::Overlays, 9, 0, 0),
                (S::LightOff, 27, 0, 0),
                (S::LightOn, 27, 0, 0),
            ],
        ),
        (
            PISTOL,
            vec![
                (S::Ready, 35, 0, 0),
                (S::Fire1, 15, 0, 0),
                (S::ReloadEmpty, 67, 0, 0),
                (S::ReloadFull, 65, 0, 0),
                (S::Melee, 45, 4, 0),
                (S::ThrowGrenade, 38, 0, 0),
                (S::Idle, 60, 0, 0),
                (S::Posing, 133, 0, 0),
                (S::Moving, 25, 0, 0),
                (S::Overlays, 9, 0, 0),
            ],
        ),
        (
            PLASMA_RIFLE,
            vec![
                (S::Ready, 29, 0, 0),
                (S::Fire1, 22, 0, 0),
                (S::Melee, 36, 6, 0),
                (S::ThrowGrenade, 37, 0, 0),
                (S::Overheated, 50, 0, 58),
                (S::Overheating, 58, 0, 0),
                (S::OverheatingAgain, 50, 0, 0),
                (S::ThrowOverheated, 35, 0, 0),
                (S::Misfire1, 18, 0, 0),
                (S::OverheatedExit, 35, 0, 0),
                (S::Idle, 60, 0, 0),
                (S::Posing, 105, 0, 0),
            ],
        ),
        (
            SHOTGUN,
            vec![
                (S::Ready, 23, 0, 0),
                (S::Fire1, 26, 0, 0),
                (S::ReloadEmpty, 12, 0, 0),
                (S::ReloadFull, 12, 0, 0),
                (S::Melee, 36, 4, 0),
                (S::ThrowGrenade, 37, 0, 0),
                (S::Enter, 15, 0, 0),
                (S::ExitEmpty, 42, 0, 0),
                (S::ExitFull, 24, 0, 0),
                (S::Idle, 62, 0, 0),
                (S::Posing, 66, 0, 0),
            ],
        ),
    ];
    for (id, slots) in table {
        let (_, graph) = fp(&file, &gun(id).tag);
        for (slot, frames, key, looping) in slots {
            let a = graph
                .first_person_animation(slot)
                .unwrap_or_else(|| panic!("{id}: no {}", slot.label()));
            assert_eq!(
                (a.frame_count, a.key_frame, a.loop_frame),
                (frames, key, looping),
                "{id}: {}",
                slot.label()
            );
        }
    }
    // The assault rifle and the plasma rifle have no fire-1 / reload.
    assert!(
        fp(&file, &gun(ASSAULT_RIFLE).tag)
            .1
            .first_person_animation(S::Fire1)
            .is_none()
    );
    assert!(
        fp(&file, &gun(PLASMA_RIFLE).tag)
            .1
            .first_person_animation(S::ReloadEmpty)
            .is_none()
    );
}

#[test]
fn the_guns_in_hand_load_from_bloodgulch() {
    let Some(file) = bloodgulch() else { return };
    let start = std::time::Instant::now();
    let looks = load::looks(&file, false);
    eprintln!(
        "the guns' looks made in {:.2} s; warnings: {:?}",
        start.elapsed().as_secs_f32(),
        looks.warnings
    );
    assert_eq!(looks.view_models.len(), 4, "{:?}", looks.warnings);
    assert_eq!(looks.held.len(), 4, "{:?}", looks.warnings);
    for v in &looks.view_models {
        let id = v.key.as_str();
        let set = v.animations.as_ref().unwrap();
        assert!(!v.model.meshes.is_empty(), "{id}: meshes");
        assert!(
            v.model.meshes.iter().all(|m| m.joints.len() == m.positions.len()),
            "{id}: skinned"
        );
        assert!(
            v.model
                .meshes
                .iter()
                .flat_map(|m| &m.joints)
                .flatten()
                .all(|j| (*j as usize) < v.bones.len()),
            "{id}: joints on the graph's nodes"
        );
        assert_eq!(v.attachments[0].name, MUZZLE, "{id}: the muzzle first");
        if id == ASSAULT_RIFLE || id == PISTOL {
            assert_eq!(v.attachments[1].name, EJECTION, "{id}: the ejection port second");
        }
        // The document's timers are the animations' lengths.
        let a = gun(id).animations;
        let frames = |s: S| sequence(set, s).map(|q| numbers(set, q));
        assert_eq!(frames(S::Ready).map(|f| f.0), Some(a.ready as usize), "{id}: the draw");
        assert_eq!(
            frames(S::Melee).map(|f| (f.0, f.2)),
            Some((a.melee as usize, a.melee_key as usize)),
            "{id}: the melee"
        );
        assert_eq!(
            frames(S::ReloadEmpty).map(|f| f.0),
            a.reload_empty.map(|r| r as usize),
            "{id}: the reload"
        );
        assert_eq!(
            frames(S::Enter).map(|f| f.0),
            a.enter.map(|r| r as usize),
            "{id}: the first shell"
        );
    }
    for h in &looks.held {
        assert_eq!(h.bone, "bip01 r hand", "{}: held in the right hand", h.key);
        assert!(h.muzzle.is_some() && !h.model.meshes.is_empty(), "{}", h.key);
    }
    // The assault rifle's idle (animations.md, the sanity check): `frame
    // gun` 0.067 WU ahead of the camera, 0.040 right and 0.089 below.
    let v = looks.view_models.iter().find(|v| v.key == ASSAULT_RIFLE).unwrap();
    let set = v.animations.as_ref().unwrap();
    let idle = sequence(set, S::Idle).unwrap();
    let pose: Vec<BonePose> = halo_ce::game::body::anims::base_pose(set, set.sequences[idle].anims[0], 0);
    let g = globals(&v.bones, &pose);
    let i = v.bones.iter().position(|b| b.name == "frame gun").unwrap();
    let at = g[i].1;
    eprintln!("frame gun at {at} (WU, Halo's axes)");
    assert!(
        (at.x - 0.067).abs() < 0.01 && (at.y + 0.040).abs() < 0.01 && (at.z + 0.089).abs() < 0.01,
        "{at}"
    );
    let eye = v.root.transform_point(at);
    assert!(
        eye.z < 0.0 && eye.x > 0.0 && eye.y < 0.0,
        "ahead, right, below in the eye: {eye}"
    );
}

#[test]
fn another_games_map_is_dressed_with_the_guns() {
    if mount().is_none() {
        return;
    }
    // The bundle's guest dressing (an app of several games, another game's
    // map): the Spartan and the four guns' view and held models.
    let mut map = chimera_world::map::MapData::default();
    let dress = halo_ce::game::BUNDLE.guest.unwrap().dress;
    dress(&mut map, &chimera_world::map::MapLoadOptions::lean()).unwrap();
    let mut guns: Vec<String> = halo_ce::game::weapons::GUNS.iter().map(|g| g.to_string()).collect();
    guns.sort();
    let mut shown: Vec<String> = map.kit.view_models.iter().map(|v| v.key.clone()).collect();
    shown.sort();
    let mut held: Vec<String> = map.kit.held.iter().map(|h| h.key.clone()).collect();
    held.sort();
    assert_eq!(shown, guns);
    assert_eq!(held, guns);
    assert_eq!(map.kit.characters.len(), 1, "the Spartan");
    let textures = map.textures.len();
    assert!(
        map.kit
            .view_models
            .iter()
            .flat_map(|v| &v.model.meshes)
            .all(|m| m.texture.is_none_or(|t| t < textures)),
        "the guns' textures appended to the map's"
    );
}
