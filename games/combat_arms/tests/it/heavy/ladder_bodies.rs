//! What a ladder's Combat Arms models cost (docs/performance.md, "Combat
//! Arms character changes"): each body record's load (`models::load_body`)
//! and the rig read of it, on the install, and whether a body's rig and its
//! model are one skeleton (`Animator::skeleton`: a body's pose goes only on
//! its own skeleton, so a difference would freeze it). Passes with a note
//! without the install.

use std::time::Instant;

use combat_arms::game::{
    GAME, INFO,
    characters::{models, skeleton::Rig},
    data::CaData,
    model,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let install = chimera_core::mount::config::LocalConfig::load().ok().and_then(|c| c.game_path(GAME));
    let Some(install) = install else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    (INFO.open)(&install).ok()
}

/// The process's resident memory (MB; Linux only, else 0).
fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map_or(0, |kb| kb / 1024)
}

#[test]
fn a_bodys_rig_and_model_are_one_skeleton() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    let data = ca.characters();
    let mut ids = data.rung_bodies();
    ids.extend(data.npc_bodies());
    ids.extend(models::LOADED);
    ids.sort();
    ids.dedup();
    let before = rss_mb();
    let (mut loaded, mut total_ms, mut max_ms, mut bytes) = (0, 0.0f32, 0.0f32, 0usize);
    for id in ids.iter().copied().step_by(if ids.len() > 40 { ids.len() / 40 } else { 1 }) {
        let r = data.model(id).unwrap();
        let started = Instant::now();
        let body = models::load_body(&ca, id, &file);
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        let Ok(body) = body else {
            eprintln!("{id:4} {:24}: didn't load ({} ms)", r.name, ms as u32);
            continue;
        };
        let size: usize = body.textures.iter().map(|t| t.rgba8.len() + t.mips.iter().map(Vec::len).sum::<usize>()).sum();
        loaded += 1;
        total_ms += ms;
        max_ms = max_ms.max(ms);
        bytes += size;
        let rig = body.index.rig(&r.file).expect("its rig registered");
        eprintln!(
            "{id:4} {:24} {:?}: load {ms:6.1} ms, {} nodes, {} textures {:.1} MB",
            r.name,
            data.family(r).stock_rig,
            rig.shape.names.len(),
            body.textures.len(),
            size as f32 / 1e6
        );
        assert_eq!(rig.shape.signature, body.model.skeleton_signature(), "{}: its rig and model", r.name);
    }
    eprintln!(
        "{loaded} bodies: mean {:.0} ms, max {max_ms:.0} ms, textures {:.1} MB each, RSS +{} MB",
        total_ms / loaded.max(1) as f32,
        bytes as f32 / 1e6 / loaded.max(1) as f32,
        rss_mb().saturating_sub(before),
    );
    // A stock group's model is on its first record's rig: its others must
    // be one skeleton with it.
    let rig_of = |id: i32| {
        let r = data.model(id).unwrap();
        model::load(&r.file, &file).ok().map(|l| (r.skeleton, r.name.clone(), Rig::new(l.model, l.list).shape.signature))
    };
    let stock: Vec<_> = models::LOADED.iter().filter_map(|id| rig_of(*id)).collect();
    for (skeleton, name, signature) in &stock {
        let first = stock.iter().find(|s| s.0 == *skeleton).unwrap();
        assert_eq!(*signature, first.2, "{name}: a stock group is one skeleton (with {})", first.1);
    }
}

/// Bodies on one skeleton hold one copy of its animation files' animations:
/// a ladder's ten bodies cost 800 MB before (docs/performance.md, "A session's
/// memory"), and they stayed for the session.
#[test]
fn bodies_share_their_animation_files() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    let data = ca.characters();
    let mut rigs = Vec::new();
    for id in data.rung_bodies().into_iter().step_by(5).take(8) {
        let Ok(body) = models::load_body(&ca, id, &file) else { continue };
        if let Some(rig) = body.index.rig(&data.model(id).unwrap().file) {
            rigs.push(rig);
        }
    }
    if rigs.len() < 2 {
        eprintln!("skipping: fewer than two bodies read");
        return;
    }
    let (files, bytes) = ca.memo(model::ChildModels::default).held();
    eprintln!("{} bodies, {files} animation files, {} MB of animations", rigs.len(), bytes >> 20);
    let mut shared = false;
    for pair in rigs.windows(2) {
        for a in &pair[0].list.entries {
            shared |= pair[1].list.entries.iter().any(|b| std::sync::Arc::ptr_eq(&a.animation, &b.animation));
        }
    }
    assert!(shared, "no two bodies share an animation: their child models are parsed for each");
    assert!(files < 8 * 8, "{files} animation files for 8 bodies: the cache isn't keyed by file");
}

/// A rung's body pool leaves out the records whose model file the install
/// lacks (`rungs::missing_bodies`: 279 and 280 on build 26236938), and
/// keeps every other one. Skips without the install.
#[test]
fn the_pool_leaves_out_bodies_with_no_file() {
    let Some(mount) = mount() else { return };
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let data = ca.characters();
    let holders = chimera_sim::minigame::holders::Holders::parse(
        r#"default = "x"
        [groups]
        x = ["@players"]"#,
    )
    .unwrap();
    let pool = combat_arms::game::rungs::bodies_for(&holders, None, Some(&ca), "combat_arms:any");
    let all = data.rung_bodies();
    let gone: Vec<i32> = all.iter().copied().filter(|&b| !mount.contains(&data.model(b).unwrap().file)).collect();
    eprintln!("{} of {} rung bodies have no file: {gone:?}", gone.len(), all.len());
    for b in &all {
        assert_eq!(pool.contains(b), !gone.contains(b), "body {b}");
    }
    assert_eq!(combat_arms::game::rungs::missing_bodies(&ca).0.iter().filter(|b| all.contains(b)).count(), gone.len());
}
