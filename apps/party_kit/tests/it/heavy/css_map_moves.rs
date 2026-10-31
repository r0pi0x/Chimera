//! Combat Arms' movement on CS:S maps (the user's installs), against
//! CS:S's (playtest 2026-10-28, cs_office): each climbs the maps' ladders
//! walking into them head-on (cs_office and de_nuke; the
//! ladder brushes are solid; Combat Arms' movement climbs them as `Ladder`
//! volumes reaching past their faces, `combat_arms`
//! `volumes::ladder_from_brush`), and each runs through a cracked
//! cs_office window shattering its panes (the panes are for shots only,
//! `source_logic` `breakables`). Whole windows stop both (spec: only a
//! broken window shatters by touch). Passes with a note without both
//! installs in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{core::MapBrushes, mount::config::LocalConfig};
use chimera_world::{
    harness::Sim,
    map::{
        MapSimPlugin,
        entities::{MapEntities, entity_rotation, entity_to_engine},
    },
};
use combat_arms::{game::movement as ca_movement, harness::CaSim};
use party_kit::PartyPartsPlugin;
use source_logic::logic::{EntId, Logic};

/// The party on map `id`; None (with a note) without both installs.
fn party_on(id: &str) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config
        .game_path(combat_arms::game::GAME)
        .filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_kit::load_map(id, 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.ticks(2);
    Some(sim)
}

const MOVEMENTS: [&str; 2] = [cs_source::game::movement::ID, ca_movement::ID];

/// A character of `movement` dropped at `at` (engine, its feet), settled;
/// None when its feet didn't come to rest between `below` m under there
/// and 0.3 m over (no floor, or pushed up onto something).
fn stand(sim: &mut Sim, movement: &'static str, at: Vec3, below: f32) -> Option<Entity> {
    let p = sim.spawn_character(at + Vec3::Y * 1.0, movement);
    sim.seconds(1.0);
    let low = sim.position(p).y + sim.state(p).hull_min.y;
    if low < at.y - below || low > at.y + 0.3 || sim.position(p).xz().distance(at.xz()) > 0.1 {
        sim.app.world_mut().despawn(p);
        return None;
    }
    Some(p)
}

/// The horizontal direction from `p` to `to` as an intent's yaw.
fn face(sim: &mut Sim, p: Entity, dir: Vec3, pitch: f32) {
    let mut i = sim.intent(p);
    i.yaw = (-dir.x).atan2(-dir.z);
    i.pitch = pitch;
}

/// Each of map `id`'s ladder brushes, walked into head-on (from 0.6 m out
/// on each side of its thin axis, looking 45 degrees up): the sides
/// CS:S's movement climbed (rose 1 m) and those of them Combat Arms'
/// didn't. None without the installs.
fn climb_ladders(id: &str) -> Option<(usize, Vec<(usize, Vec3, f32, f32)>)> {
    let mut sim = party_on(id)?;
    let ladders: Vec<_> = sim
        .app
        .world()
        .resource::<MapBrushes>()
        .0
        .iter()
        .filter(|b| b.ladder)
        .cloned()
        .collect();
    let mut tried = 0;
    let mut failed = Vec::new();
    for (k, b) in ladders.iter().enumerate() {
        let size = b.max - b.min;
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let centre = (b.min + b.max) / 2.0;
        for side in [1.0, -1.0] {
            let out = thin * side * (size.dot(thin) / 2.0 + 0.6);
            // The ladder brush's foot may be under the floor.
            let feet = Vec3::new(centre.x + out.x, b.min.y + 1.5, centre.z + out.z);
            let mut climbed = Vec::new();
            for movement in MOVEMENTS {
                let Some(p) = stand(&mut sim, movement, feet, 2.0) else {
                    continue;
                };
                face(&mut sim, p, -thin * side, std::f32::consts::FRAC_PI_4);
                let start = sim.position(p).y;
                sim.intent(p).move_axis = Vec2::Y;
                sim.seconds(3.0);
                climbed.push((movement, sim.position(p).y - start));
                sim.app.world_mut().despawn(p);
            }
            if climbed.len() < 2 {
                continue;
            }
            eprintln!("{id} ladder {k} at {centre:.2} side {side}: {climbed:.2?}");
            // Only where CS:S climbs: a ladder side against a wall isn't one.
            if climbed[0].1 < 1.0 {
                continue;
            }
            tried += 1;
            if climbed[1].1 < 1.0 {
                failed.push((k, centre, side, climbed[1].1));
            }
        }
    }
    Some((tried, failed))
}

#[test]
fn both_movements_climb_cs_office_ladders_head_on() {
    let Some((tried, failed)) = climb_ladders("cs_source:cs_office") else {
        return;
    };
    assert!(tried > 0, "some ladder side CS:S's movement climbs");
    assert!(
        failed.is_empty(),
        "Combat Arms' movement didn't climb {failed:?} of {tried}"
    );
}

/// de_dust2 and cs_italy have no ladder CS:S climbs this way; de_train's
/// (checked 2026-10-28: Combat Arms climbed every side CS:S did) take
/// minutes.
#[test]
fn both_movements_climb_de_nuke_ladders_head_on() {
    let Some((tried, failed)) = climb_ladders("cs_source:de_nuke") else {
        return;
    };
    assert!(tried > 0, "some ladder side CS:S's movement climbs");
    assert!(
        failed.is_empty(),
        "Combat Arms' movement didn't climb {failed:?} of {tried}"
    );
}

/// The map's breakables, (index, engine bounds).
fn glass(sim: &Sim) -> Vec<(usize, Vec3, Vec3)> {
    let entities = sim.app.world().resource::<MapEntities>().clone();
    entities
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf" || e.classname() == "func_breakable")
        .map(|(i, e)| {
            let rot = entity_rotation(e.angles());
            let (lo, hi) = e
                .hulls
                .iter()
                .flat_map(|h| &h.points)
                .map(|p| entity_to_engine(rot * *p + e.origin(), entities.scale))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
            (i, lo, hi)
        })
        .collect()
}

/// Map entity `index`'s window: (its logic id, panes broken).
fn window_state(sim: &Sim, index: usize) -> (EntId, Option<usize>) {
    let logic = sim.app.world().resource::<Logic>();
    let id = logic
        .world
        .ids()
        .into_iter()
        .find(|id| logic.world.get(*id).unwrap().map_index == Some(index))
        .expect("the window");
    (
        id,
        logic
            .world
            .window(id)
            .filter(|w| w.window_broken)
            .map(|w| w.broken_count()),
    )
}

/// A cracked window (one bullet in its top corner: broken as an entity,
/// its other panes still standing) run through square: each movement
/// shatters the panes its box touches (breakables.md "Touch").
#[test]
fn both_movements_shatter_a_cracked_cs_office_window_running_through_it() {
    let Some(mut sim) = party_on("cs_source:cs_office") else {
        return;
    };
    let scale = sim.app.world().resource::<MapEntities>().scale;
    let panes: Vec<_> = glass(&sim)
        .into_iter()
        .filter(|(i, ..)| sim.app.world().resource::<MapEntities>().entities[*i].classname() == "func_breakable_surf")
        .collect();
    let mut tried = 0;
    let mut failed = Vec::new();
    for &(i, lo, hi) in &panes {
        let size = hi - lo;
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let along = if thin == Vec3::X { Vec3::Z } else { Vec3::X };
        let centre = (lo + hi) / 2.0;
        for side in [1.0, -1.0] {
            let out = thin * side * (size.dot(thin) / 2.0 + 2.0);
            let mut result = Vec::new();
            for movement in MOVEMENTS {
                chimera_core::core::restart_round(sim.app.world_mut());
                sim.ticks(3);
                let Some(p) = stand(
                    &mut sim,
                    movement,
                    Vec3::new(centre.x + out.x, lo.y, centre.z + out.z),
                    1.5,
                ) else {
                    continue;
                };
                // Crack it: a bullet in a top corner pane.
                let corner = centre + along * (size.dot(along) / 2.0 - 0.1) + Vec3::Y * (size.y / 2.0 - 0.1);
                let (id, _) = window_state(&sim, i);
                {
                    let mut logic = sim.app.world_mut().resource_mut::<Logic>();
                    let to_entity = |v: Vec3| Vec3::new(v.x, -v.z, v.y) / scale;
                    let dir = to_entity(-thin * side).normalize();
                    logic.world.damage(
                        id,
                        1.0,
                        chimera_core::core::DamageKind::Bullet,
                        None,
                        to_entity(corner),
                        dir,
                    );
                }
                sim.ticks(2);
                let Some(cracked) = window_state(&sim, i).1 else {
                    panic!("window {i} cracked by the bullet");
                };
                face(&mut sim, p, -thin * side, 0.0);
                sim.intent(p).move_axis = Vec2::Y;
                sim.seconds(2.0);
                let gap = (sim.position(p) - centre).dot(thin * side);
                let after = window_state(&sim, i).1.unwrap_or(usize::MAX);
                eprintln!("  {movement}: cracked {cracked}, after the run {after}, ends {gap:.2} m past the middle");
                result.push((movement, after > cracked + 1, gap < 0.0));
                sim.app.world_mut().despawn(p);
            }
            if result.len() < 2 {
                continue;
            }
            eprintln!("window {i} at {centre:.2} side {side}: {result:?}");
            if !result[0].1 {
                continue;
            }
            tried += 1;
            if !result[1].1 {
                failed.push((i, centre, side));
            }
        }
    }
    assert!(
        tried > 0,
        "CS:S's movement shatters some cracked window running through it"
    );
    assert!(
        failed.is_empty(),
        "Combat Arms' movement didn't shatter {failed:?} of {tried}"
    );
}
