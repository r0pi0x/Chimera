//! Every game's guns eject their own casings on every game's map (the
//! installs'): on the party's de_dust2, Warehouse and dm_lockdown
//! (`party_kit::load_map`, each dressed with the other games' parts), a
//! first-person CS:S AK-47 throws CS:S's 7.62 casing with CS:S's physics,
//! HL2:DM's SMG1 HL2:DM's shell with HL2:DM's, and Combat Arms' M4A1 its
//! own casing (`effects::CaShell`). Before, a map held one game's shell
//! types and physics, its own game's, and only with its whole kit: in the
//! party (Combat Arms' gameplay) neither Source game's guns ejected
//! anything on any map. Passes with a note without the three installs.

use bevy::prelude::*;
use chimera_core::{
    core::{LocalPlayer, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_present::{
    MapPlugin,
    render::{
        decal::DecalMaterial, prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial,
        shells::Shell, sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_sim::testkit;
use chimera_world::{
    harness::Sim,
    map::{ViewAnimator, ViewModelAnchor, shells::MapShellPhysics},
};
use combat_arms::{game::effects::CaShell, harness::CaSim};
use party_kit::PartyPartsPlugin;

const MAPS: [&str; 3] = ["cs_source:de_dust2", "combat_arms:warehouse", "hl2dm:dm_lockdown"];

fn render_assets(app: &mut App) {
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<WorldMaterial>()
        .init_asset::<PropMaterial>()
        .init_asset::<SpriteMaterial>()
        .init_asset::<RopeMaterial>()
        .init_asset::<DecalMaterial>()
        .init_asset::<ShadowMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
}

fn installed() -> bool {
    let config = LocalConfig::load().ok();
    let usable = config.is_some_and(|c| {
        c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
            .is_some_and(|p| chimera_core::mount::served::usable("cs_source", &p, "cstrike"))
            && c.game_path(combat_arms::game::GAME)
                .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
    }) && hl2dm::game::INFO.install().is_some();
    if !usable {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
    }
    usable
}

/// The local player at the map's first spawn, seen in first person (a
/// view-model anchor on it), looking level.
fn local_player(sim: &mut Sim) -> Entity {
    let at = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Transform, &SpawnPoint)>();
        q.iter(w).next().map(|(t, _)| t.translation).expect("a spawn point")
    };
    let p = sim.spawn_character(at, combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, ViewAnimator::default()));
    sim.app.world_mut().spawn((
        Camera3d::default(),
        ViewModelAnchor,
        Transform::from_translation(Vec3::Y * 0.6),
        Visibility::default(),
        ChildOf(p),
    ));
    sim.intent(p).pitch = 0.0;
    sim.seconds(1.0);
    p
}

/// The casings `gun` ejects in a short burst: the Source games' (their
/// kinds and physics), and Combat Arms' count.
fn burst(sim: &mut Sim, p: Entity, gun: &str) -> (Vec<(String, MapShellPhysics)>, usize) {
    chimera_sim::weapon::give(sim.app.world_mut(), p, gun).unwrap_or_else(|| panic!("{gun} given"));
    sim.seconds(2.0);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(gun));
    let mut source = Vec::new();
    let mut ca = 0;
    sim.intent(p).fire = true;
    for _ in 0..40 {
        sim.ticks(1);
        let w = sim.app.world_mut();
        for s in w.query::<&Shell>().iter(w) {
            if !source.iter().any(|(k, _)| k == s.kind()) {
                source.push((s.kind().to_string(), *s.physics()));
            }
        }
        ca = ca.max(w.query::<&CaShell>().iter(w).count());
    }
    sim.intent(p).fire = false;
    sim.seconds(15.0);
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&Shell>().iter(w).count(), 0, "{gun}: its casings gone after their life");
    (source, ca)
}

#[test]
fn every_games_guns_eject_their_own_casings_on_every_map() {
    if !installed() {
        return;
    }
    let cs = cs_source::game::view_anim::shell_physics();
    let hl2 = hl2dm::game::shells::physics();
    let mut problems = Vec::new();
    for map in MAPS {
        let data = party_kit::load_map(map, 0).unwrap();
        let mut types: Vec<String> = data.kit.shells.iter().map(|s| format!("{}:{}", s.game, s.key)).collect();
        types.sort();
        eprintln!("{map}: shell types {types:?}");
        let mut sim = Sim::new(CaSim, (render_assets, MapPlugin::new(data), PartyPartsPlugin));
        sim.ticks(2);
        let p = local_player(&mut sim);
        for (gun, kind, physics) in [
            ("cs_source:weapon_ak47", Some("cs_source:762nato"), Some(cs)),
            ("hl2dm:weapon_smg1", Some("hl2dm:0"), Some(hl2)),
            ("combat_arms:m4a1", None, None),
        ] {
            let (source, ca) = burst(&mut sim, p, gun);
            eprintln!("{map} {gun}: {source:?} kinds, {ca} Combat Arms casings");
            match (kind, physics) {
                (Some(kind), Some(physics)) => {
                    if source.is_empty() {
                        problems.push(format!("{map}: {gun} ejected nothing"));
                    }
                    for (k, ph) in &source {
                        if k != kind || *ph != physics {
                            problems.push(format!("{map}: {gun} ejected {k} (wanted {kind}), its physics {}", if *ph == physics { "its game's" } else { "another's" }));
                        }
                    }
                    if ca > 0 {
                        problems.push(format!("{map}: {gun} threw Combat Arms casings"));
                    }
                }
                _ => {
                    if ca == 0 {
                        problems.push(format!("{map}: {gun} ejected no casing"));
                    }
                    if !source.is_empty() {
                        problems.push(format!("{map}: {gun} threw Source casings {source:?}"));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
