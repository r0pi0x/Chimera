//! Bullet impacts across games on the real maps (the user's installs;
//! `chimera_sim::weapon::impact`): in the party, a CS:S gun fired at a
//! wall of Combat Arms' Warehouse leaves CS:S's impact (its decal, sound
//! and debris, from CS:S's looks the map carries as a guest's, the wall's
//! Combat Arms surface as CS:S's by class), a Combat Arms gun on de_dust2
//! leaves Combat Arms' (its impact sound at the hit), and each game's
//! own pairing (a CS:S gun on de_dust2, a Combat Arms gun on Warehouse)
//! still does; every impact is drawn by one game only. Passes with a
//! note without both installs in chimera.local.toml.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::SystemState, prelude::*};
use chimera_core::{
    core::{SOLID_LAYERS, SpawnPoint},
    mount::config::LocalConfig,
};
use chimera_sim::{testkit, weapon::impact::ImpactRoute};
use chimera_world::{
    harness::Sim,
    map::{
        DecalGroup, MapSimPlugin, PlaceDecal, PlaySound,
        impact::ActiveImpacts,
        particles::Particles,
    },
};
use combat_arms::{
    game::{effects::sounds::CaSound, movement},
    harness::CaSim,
};
use party_kit::PartyPartsPlugin;

/// What the impact systems wrote, kept for the test.
#[derive(Resource, Default)]
struct Seen {
    decals: Vec<PlaceDecal>,
    sounds: Vec<PlaySound>,
    ca: Vec<CaSound>,
    /// The most particles alive in the half second after the round.
    particles: usize,
}

fn record(
    mut decals: MessageReader<PlaceDecal>,
    mut sounds: MessageReader<PlaySound>,
    mut ca: MessageReader<CaSound>,
    mut seen: ResMut<Seen>,
) {
    seen.decals.extend(decals.read().cloned());
    seen.sounds.extend(sounds.read().cloned());
    seen.ca.extend(ca.read().cloned());
}

/// The party on map `id`; None (with a note) without both installs.
fn party_on(id: &str) -> Option<Sim> {
    let config = LocalConfig::load().ok()?;
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if css.is_none() || ca.is_none() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return None;
    }
    let map = party_kit::load_map(id, 0).unwrap();
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.app.init_resource::<Seen>().add_systems(Last, record);
    sim.ticks(2);
    Some(sim)
}

/// A soldier at the map's first spawn holding `gun`, and a point of the
/// world 2-25 m from its eye along one of eight level directions.
fn shooter_facing_a_wall(sim: &mut Sim, gun: &str) -> (Entity, Vec3) {
    let at = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(&Transform, &SpawnPoint)>();
        q.iter(w).next().map(|(t, _)| t.translation).expect("a spawn point")
    };
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    chimera_sim::weapon::give(sim.app.world_mut(), p, gun);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(gun));
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let mut state: SystemState<SpatialQuery> = SystemState::new(sim.app.world_mut());
    let spatial = state.get(sim.app.world()).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([p]).with_mask(SOLID_LAYERS);
    let wall = (0..8)
        .filter_map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            let dir = Dir3::new(Vec3::new(a.cos(), 0.0, a.sin())).ok()?;
            let hit = spatial.cast_ray(eye, dir, 25.0, true, &filter)?;
            (hit.distance > 2.0).then(|| eye + *dir * hit.distance)
        })
        .next()
        .expect("a wall within 25 m");
    (p, wall)
}

/// One round from `p` at `wall`; what was written in the next half
/// second.
fn fire_at(sim: &mut Sim, p: Entity, wall: Vec3) -> Seen {
    testkit::aim_at(sim, p, wall);
    sim.ticks(2);
    sim.app.world_mut().insert_resource(Seen::default());
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let mut most = 0;
    for _ in 0..(0.5 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        most = most.max(sim.app.world().resource::<Particles>().count());
    }
    let mut seen = sim.app.world_mut().remove_resource::<Seen>().unwrap();
    seen.particles = most;
    seen
}

fn near(at: Option<Vec3>, wall: Vec3) -> bool {
    at.is_some_and(|a| a.distance(wall) < 0.5)
}

fn route(sim: &mut Sim, p: Entity) -> chimera_sim::weapon::impact::ImpactBy {
    let gun = testkit::active(sim.app.world(), p).unwrap();
    let mut state: SystemState<ImpactRoute> = SystemState::new(sim.app.world_mut());
    state.get(sim.app.world()).unwrap().by_weapon(gun)
}

#[test]
fn a_cs_source_gun_on_warehouse_leaves_cs_sources_impact() {
    let Some(mut sim) = party_on("combat_arms:warehouse") else { return };
    let impacts = sim.app.world().resource::<ActiveImpacts>().0.clone();
    assert_eq!(impacts.game.as_deref(), Some("combat_arms"));
    assert!(impacts.has("combat_arms") && impacts.has("cs_source"), "CS:S's looks as a guest's: {impacts:?}");
    let (p, wall) = shooter_facing_a_wall(&mut sim, "cs_source:weapon_ak47");
    assert_eq!(route(&mut sim, p), chimera_sim::weapon::impact::ImpactBy::Game("cs_source"));
    let before = sim.app.world().resource::<Particles>().count();
    let seen = fire_at(&mut sim, p, wall);
    let decal = seen.decals.iter().find(|d| d.point.distance(wall) < 0.5);
    assert!(
        matches!(decal.map(|d| &d.group), Some(DecalGroup::Material(_) | DecalGroup::Named(_))),
        "CS:S's bullet hole on the wall: {:?}",
        seen.decals
    );
    assert!(
        seen.sounds.iter().any(|s| near(s.at, wall)),
        "CS:S's impact sound at the wall: {:?}",
        seen.sounds.iter().map(|s| &s.entry).collect::<Vec<_>>()
    );
    let mats = sim.app.world().resource::<chimera_world::map::particles::ParticleMaterials>();
    assert!(mats.0.find("effects/fleck_cement1").is_some(), "CS:S's effect materials on the map");
    assert!(seen.particles > before, "CS:S's debris or sparks");
    assert!(!seen.ca.iter().any(|s| near(s.at, wall)), "drawn once: no Combat Arms impact too");
}

#[test]
fn a_combat_arms_gun_on_dust2_leaves_combat_arms_impact() {
    let Some(mut sim) = party_on("cs_source:de_dust2") else { return };
    let (p, wall) = shooter_facing_a_wall(&mut sim, "combat_arms:m4a1");
    assert_eq!(route(&mut sim, p), chimera_sim::weapon::impact::ImpactBy::Game("combat_arms"));
    let seen = fire_at(&mut sim, p, wall);
    assert!(
        seen.ca.iter().any(|s| near(s.at, wall)),
        "Combat Arms' impact sound at the wall: {:?}",
        seen.ca.iter().map(|s| (&s.file, s.at)).collect::<Vec<_>>()
    );
    assert!(!seen.decals.iter().any(|d| d.point.distance(wall) < 0.5), "drawn once: no CS:S decal too");
}

#[test]
fn each_games_own_gun_on_its_own_map_still_leaves_its_impact() {
    for (map, gun, game) in [
        ("cs_source:de_dust2", "cs_source:weapon_ak47", "cs_source"),
        ("combat_arms:warehouse", "combat_arms:m4a1", "combat_arms"),
    ] {
        let Some(mut sim) = party_on(map) else { return };
        let (p, wall) = shooter_facing_a_wall(&mut sim, gun);
        assert_eq!(route(&mut sim, p), chimera_sim::weapon::impact::ImpactBy::Game(game), "{map}");
        let seen = fire_at(&mut sim, p, wall);
        let cs = seen.decals.iter().any(|d| d.point.distance(wall) < 0.5);
        let ca = seen.ca.iter().any(|s| near(s.at, wall));
        assert_eq!((cs, ca), (game == "cs_source", game == "combat_arms"), "{map} {gun}");
    }
}
