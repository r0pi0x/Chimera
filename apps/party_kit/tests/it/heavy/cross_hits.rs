//! Hits across games on the real bodies (the user's installs; docs/
//! DESIGN.md "Hits across games"; playtest 2026-10-28): in the party on
//! de_dust2, a Combat Arms round aimed at the centre of a CS:S player's
//! head box (its model's hitbox set, posed) is a head shot with head
//! damage, as is a CS:S round at a Combat Arms soldier's head sphere.
//! Passes with a note without both installs in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{
    core::{BodyRegion, Health, Hitboxes, Hitgroup},
    mount::config::LocalConfig,
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

use crate::{
    cross_game::{Hits, collect_hits, health_and_armour, set_rung, shot_at},
    gun_game::{held, place, soldier},
};

/// The gun game on de_dust2 with a shooter on rung `shooter` and a target
/// on rung `target` 4.9 m apart (CT spawns 0 and 2), facing each other;
/// None (with a note) without both installs.
fn duel_on_dust2(shooter: usize, target: usize) -> Option<(Sim, Entity, Entity)> {
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    let (pa, pb) = (map.spawns[2].0, map.spawns[0].0);
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    // No rung extras or finale: they draw random melee weapons, and the
    // knife test needs rung 7's own (m9_knife).
    crate::gun_game::plain_rungs(sim.app.world_mut());
    crate::gun_game::no_finale(sim.app.world_mut());
    chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME).expect("the gun game starts");
    sim.app.init_resource::<Hits>().add_systems(Update, collect_hits);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    place(&mut sim, a, pa + Vec3::Y * 1.0);
    place(&mut sim, b, pb + Vec3::Y * 1.0);
    set_rung(&mut sim, a, shooter);
    set_rung(&mut sim, b, target);
    sim.seconds(1.5);
    let d = sim.position(a) - sim.position(b);
    sim.intent(b).yaw = (-d.x).atan2(-d.z);
    sim.seconds(1.0);
    let mut h = sim.app.world_mut().get_mut::<Health>(b).unwrap();
    h.current = 100.0;
    h.max = 100.0;
    Some((sim, a, b))
}

/// Where the centre of `b`'s highest shape of `region` is in the world
/// (CS:S's head group has its neck box too).
fn top_shape_at(sim: &Sim, b: Entity, region: BodyRegion) -> Vec3 {
    let w = sim.app.world();
    let boxes = w.get::<Hitboxes>(b).expect("hitboxes");
    let shape = boxes
        .0
        .iter()
        .filter(|h| h.region == region)
        .max_by(|x, y| x.center.y.total_cmp(&y.center.y))
        .unwrap_or_else(|| panic!("no {region:?}"));
    let t = w.get::<Transform>(b).unwrap().translation;
    let feet = w.get::<avian3d::prelude::ColliderAabb>(b).map_or(t, |a| t.with_y(a.min.y));
    feet + w.get::<chimera_core::core::Intent>(b).unwrap().yaw_rotation() * shape.center
}

/// One round at the centre of `b`'s highest shape of `region`: the
/// hitgroups it hit and the health it took (points).
fn round_at(sim: &mut Sim, a: Entity, b: Entity, region: BodyRegion) -> (Vec<Hitgroup>, f32) {
    let at = top_shape_at(sim, b, region);
    let (h0, _) = health_and_armour(sim, b);
    let hits = shot_at(sim, a, b, at);
    let (h1, _) = health_and_armour(sim, b);
    let mut h = sim.app.world_mut().get_mut::<Health>(b).unwrap();
    h.current = 100.0;
    (hits, h0 - h1)
}

#[test]
fn a_combat_arms_round_on_a_cs_source_head_box_is_a_head_shot() {
    // Rung 7: Combat Arms' M4A1 at rung 6's CS:S player (CS:S's model
    // and its hitbox set).
    let Some((mut sim, a, b)) = duel_on_dust2(7, 6) else { return };
    assert_eq!(held(&sim, a), Some("combat_arms:m4a1"));
    let boxes = sim.app.world().get::<Hitboxes>(b).cloned().expect("the CS:S body's hitboxes");
    assert!(boxes.0.iter().all(|h| h.radius.is_none()), "CS:S's boxes");
    let (head, head_damage) = round_at(&mut sim, a, b, BodyRegion::Head);
    let (chest, chest_damage) = round_at(&mut sim, a, b, BodyRegion::Chest);
    let w = sim.app.world();
    let top = w.get::<avian3d::prelude::ColliderAabb>(b).unwrap().max.y;
    eprintln!("head centre {:.3} m, hull top {top:.3} m", top_shape_at(&sim, b, BodyRegion::Head).y);
    assert_eq!(head, vec![Hitgroup::Head], "the head box: {boxes:?}");
    // (The idle pose holds the arms across the chest.)
    assert!(!chest.is_empty() && !chest.contains(&Hitgroup::Head), "{chest:?}");
    assert!(head_damage > chest_damage * 1.5, "head {head_damage}, chest {chest_damage}");
}

#[test]
fn a_cs_source_round_on_a_combat_arms_head_sphere_is_a_head_shot() {
    // Rung 6: CS:S's AK-47 at rung 7's Combat Arms soldier.
    let Some((mut sim, a, b)) = duel_on_dust2(6, 7) else { return };
    assert_eq!(held(&sim, a), Some("cs_source:weapon_ak47"));
    let (head, head_damage) = round_at(&mut sim, a, b, BodyRegion::Head);
    let (chest, chest_damage) = round_at(&mut sim, a, b, BodyRegion::Chest);
    assert_eq!(head, vec![Hitgroup::Head]);
    assert!(!chest.is_empty() && !chest.contains(&Hitgroup::Head), "{chest:?}");
    assert!(head_damage > chest_damage * 1.5, "head {head_damage}, chest {chest_damage}");
}

/// Every region a hit reported: (target, region).
#[derive(Resource, Default)]
struct Regions(Vec<(Entity, BodyRegion)>);

fn collect_regions(mut hits: MessageReader<chimera_core::core::RegionHit>, mut out: ResMut<Regions>) {
    out.0.extend(hits.read().map(|h| (h.target, h.region)));
}

#[test]
fn a_combat_arms_knife_on_a_cs_source_head_box_is_a_head_hit() {
    // Rung 7's Combat Arms soldier with its knife (class key 3), up close
    // to rung 6's CS:S player: the CS:S model's head box, not Combat Arms'
    // skeleton it keeps posing out of sight.
    let Some((mut sim, a, b)) = duel_on_dust2(7, 6) else { return };
    sim.app.init_resource::<Regions>().add_systems(PostUpdate, collect_regions);
    let (pa, pb) = (sim.position(a), sim.position(b));
    let close = pb + (pa - pb).with_y(0.0).normalize() * 0.9;
    place(&mut sim, a, close.with_y(pa.y));
    sim.intent(a).select = Some(2);
    sim.ticks(1);
    sim.intent(a).select = None;
    sim.seconds(1.0);
    assert_eq!(held(&sim, a), Some(combat_arms::game::inventory::KNIFE));
    let head = top_shape_at(&sim, b, BodyRegion::Head);
    chimera_sim::testkit::aim_at(&mut sim, a, head);
    sim.ticks(1);
    sim.intent(a).fire = true;
    sim.ticks(1);
    sim.intent(a).fire = false;
    sim.seconds(1.0);
    let regions: Vec<BodyRegion> =
        sim.app.world().resource::<Regions>().0.iter().filter(|(t, _)| *t == b).map(|(_, r)| *r).collect();
    assert_eq!(regions.first(), Some(&BodyRegion::Head), "{regions:?}");
}
