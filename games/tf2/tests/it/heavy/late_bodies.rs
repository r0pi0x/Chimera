//! TF2's bodies that come late (`tf2::game::late_bodies`), from the
//! user's install behind a gate that plays the asset store before its
//! records arrive (the files listed, not readable): the Heavy's model and
//! every lightwarp. A Heavy worn on a map that has only the Scout's body
//! waits for its files, then loads as a late body once a TF2 unit arrives;
//! the Scout's body, built while its lightwarp was late (recorded in
//! `MapModel::late_files`, its skin without the warp), is built again
//! with it and replaces the old one by name. Passes with a note without
//! the install.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{prelude::*, tasks::{AsyncComputeTaskPool, TaskPool}};
use chimera_core::{
    mount::{Entry, FileSource, Mount},
    slots::GameIdentity,
    tasks::WaitForTasks,
};
use chimera_net::net::units::{UnitChanged, UnitState, UnitStatus};
use chimera_world::map::{BodyName, CharacterModels, LateCharacterModels, MapCharacterModel, REPLACED, append_late_characters};
use source_engine::content::{material::MaterialLoader, surfaceprops::SurfaceProps};
use tf2::game::{
    GAME,
    bodies,
    classes::Classes,
    late_bodies::{LateBodies, Tf2Files, late_bodies},
};

use super::maps::install_mount;

const HEAVY: &str = "models/player/heavy.mdl";
const SCOUT: &str = "models/player/scout.mdl";

/// The store's gate: while closed, the gated files are listed (the
/// install has them) but don't read and aren't held.
struct Gate {
    closed: Arc<AtomicBool>,
    install: Arc<Mount>,
}

fn gated(path: &str) -> bool {
    path.contains("lightwarp") || path.starts_with("models/player/heavy.")
}

impl FileSource for Gate {
    fn name(&self) -> String {
        "gate".into()
    }
    fn entries(&self) -> Vec<Entry> {
        Vec::new()
    }
    fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
        self.contains(path).then(|| Err(std::io::Error::other(format!("asset miss: {path}"))))
    }
    fn contains(&self, path: &str) -> bool {
        self.closed.load(Ordering::SeqCst) && gated(path) && self.install.contains(path)
    }
    fn held(&self, _: &str) -> bool {
        false
    }
}

fn gated_mount(closed: &Arc<AtomicBool>) -> Result<Mount, String> {
    let install = install_mount().ok_or("no install")?;
    let mut m = Mount::default();
    m.push(Gate { closed: closed.clone(), install: Arc::new(install_mount().ok_or("no install")?) });
    for l in install.into_layers() {
        m.push_boxed(l);
    }
    Ok(m)
}

fn has_warp(m: &MapCharacterModel) -> bool {
    m.model.meshes.iter().any(|mesh| mesh.phong.as_ref().is_some_and(|p| p.lightwarp.is_some()))
}

#[test]
fn a_class_picked_late_loads_and_a_body_missing_its_lightwarp_is_built_again() {
    if install_mount().is_none() {
        eprintln!("note: no TF2 install; skipped");
        return;
    }
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let closed = Arc::new(AtomicBool::new(true));
    // The map's TF2 body: the Scout's, built while the lightwarp was late.
    let mount = gated_mount(&closed).unwrap();
    let mut materials = MaterialLoader::guest(&mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    let scout = bodies::load(&mut materials, &surfaces, SCOUT, "").unwrap();
    assert!(scout.model.late_files.iter().any(|f| f.contains("lightwarp")), "late: {:?}", scout.model.late_files);
    assert!(!has_warp(&scout), "built without its lightwarp");
    drop(materials);

    let mut app = App::new();
    let files = closed.clone();
    app.add_plugins(MinimalPlugins)
        .add_message::<UnitChanged>()
        .insert_resource(WaitForTasks)
        .init_resource::<Classes>()
        .init_resource::<LateBodies>()
        .insert_resource(Tf2Files(Arc::new(move || gated_mount(&files))))
        .insert_resource(CharacterModels(Arc::new(vec![scout])))
        .add_systems(PreUpdate, append_late_characters)
        .add_systems(Update, late_bodies);
    let heavy = app.world_mut().spawn((GameIdentity(GAME.into()), BodyName(HEAVY.into()))).id();
    app.world_mut().spawn((GameIdentity(GAME.into()), BodyName(SCOUT.into())));
    for _ in 0..3 {
        app.update();
    }
    // The Heavy's model isn't here: it waits; nothing added yet.
    assert!(app.world().resource::<LateBodies>().waiting(HEAVY));
    assert!(app.world().get_resource::<LateCharacterModels>().is_none_or(|l| l.get(HEAVY).is_none()));

    // The files arrive (a TF2 unit is ready).
    closed.store(false, Ordering::SeqCst);
    app.world_mut().write_message(UnitChanged {
        game: GAME.into(),
        set: "players/heavy".into(),
        status: UnitStatus { state: UnitState::Ready, ..default() },
    });
    for _ in 0..4 {
        app.update();
    }
    let state = app.world().resource::<LateBodies>();
    assert!(state.failed().is_empty(), "{:?}", state.failed());
    assert_eq!((state.added, state.replaced), (1, 1));
    let models = app.world().resource::<CharacterModels>().clone();
    let names: Vec<_> = models.0.iter().map(|m| m.name.clone()).collect();
    let find = |n: &str| models.0.iter().find(|m| m.name.as_deref() == Some(n));
    let heavy_body = find(HEAVY).unwrap_or_else(|| panic!("the Heavy's body: {names:?}"));
    assert!(has_warp(heavy_body) && heavy_body.model.late_files.is_empty());
    let new_scout = find(SCOUT).unwrap_or_else(|| panic!("the Scout's: {names:?}"));
    assert!(has_warp(new_scout), "the Scout's body built again with its lightwarp");
    assert!(new_scout.model.late_files.is_empty());
    assert!(find(&format!("{SCOUT}{REPLACED}")).is_some(), "the old Scout renamed: {names:?}");
    // The Heavy's wearer finds its body by name now.
    let identity = GameIdentity(GAME.into());
    let worn = models.for_identity(Some(&identity), None, app.world().get::<BodyName>(heavy), None).map(|(_, m)| m.name.clone());
    assert_eq!(worn, Some(Some(HEAVY.to_string())));
}
