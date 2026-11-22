//! Zones a player must not be placed in, registered per game and read by
//! whatever places players on a map (the minigames' random spawns,
//! `chimera_sim::minigame::spawns`): a function returning the world-space
//! brushes (engine space, m) that hurt, kill or drown a player standing
//! in them (CS:S's `trigger_hurt`, Combat Arms' damage and liquid
//! volumes). Water volumes (`core::MapWater`) count without registering.
//! Called when the zones are wanted, so a source sees the map's current
//! state. Per game, how high the spots' walker climbs on its maps
//! (`SpawnClimbs`), and which of its maps' volumes are team spawn rooms
//! (`SpawnRooms`: TF2's `func_respawnroom`), which a free-for-all's spawns
//! keep out of (`spawn_room_zones`).

use bevy::prelude::*;

use crate::core::MapBrush;

/// The registered sources.
#[derive(Resource, Default)]
pub struct SpawnHazards(pub Vec<fn(&mut World) -> Vec<MapBrush>>);

/// Registering hazards.
pub trait SpawnHazardAppExt {
    fn add_spawn_hazards(&mut self, source: fn(&mut World) -> Vec<MapBrush>) -> &mut Self;
}

impl SpawnHazardAppExt for App {
    fn add_spawn_hazards(&mut self, source: fn(&mut World) -> Vec<MapBrush>) -> &mut Self {
        let mut hazards = self.world_mut().get_resource_or_init::<SpawnHazards>();
        if !hazards.0.iter().any(|s| *s as usize == source as usize) {
            hazards.0.push(source);
        }
        self
    }
}

/// How high the spawn spots' walker climbs on a game's maps (m), per game
/// (the map's game id, `core::MapGame`): a game whose maps' players get
/// up ledges higher than a walk's step (Combat Arms': sills a little
/// above its stair step, jumped up; the warehouse's spawn rooms' 0.7 m
/// sill once kept every spot in the two rooms). A game not listed: a
/// walking player's step (`nav_gen::walking_player`).
#[derive(Resource, Default)]
pub struct SpawnClimbs(pub Vec<(&'static str, f32)>);

impl SpawnClimbs {
    /// The climb registered for `game`'s maps.
    pub fn of(&self, game: &str) -> Option<f32> {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map(|(_, h)| *h)
    }
}

/// Registering a game's climb.
pub trait SpawnClimbAppExt {
    fn add_spawn_climb(&mut self, game: &'static str, height: f32) -> &mut Self;
}

impl SpawnClimbAppExt for App {
    fn add_spawn_climb(&mut self, game: &'static str, height: f32) -> &mut Self {
        let mut climbs = self.world_mut().get_resource_or_init::<SpawnClimbs>();
        climbs.0.retain(|(g, _)| *g != game);
        climbs.0.push((game, height));
        self
    }
}

/// Per game, the classes of its maps' entities whose volumes are team
/// spawn rooms (TF2's `func_respawnroom`: a team's spawn points, its
/// resupply cabinet, doors only its team opens). A free-for-all's spawns
/// anywhere on the map (`chimera_sim::minigame::spawns`) keep out of them,
/// as out of hazards (`spawn_room_zones`); a team mode's spawns at its
/// team's points are unaffected. A game not listed has none.
#[derive(Resource, Default)]
pub struct SpawnRooms(pub Vec<(&'static str, &'static [&'static str])>);

impl SpawnRooms {
    /// The spawn-room classes registered for `game`'s maps.
    pub fn of(&self, game: &str) -> &'static [&'static str] {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map_or(&[], |(_, c)| *c)
    }
}

/// Registering a game's spawn-room classes.
pub trait SpawnRoomAppExt {
    fn add_spawn_rooms(&mut self, game: &'static str, classes: &'static [&'static str]) -> &mut Self;
}

impl SpawnRoomAppExt for App {
    fn add_spawn_rooms(&mut self, game: &'static str, classes: &'static [&'static str]) -> &mut Self {
        let mut rooms = self.world_mut().get_resource_or_init::<SpawnRooms>();
        rooms.0.retain(|(g, _)| *g != game);
        rooms.0.push((game, classes));
        self
    }
}

/// The loaded map's spawn rooms in engine space (m): the volumes of its
/// entities (`map::MapEntities`) of the classes its game (`core::MapGame`)
/// registered (`SpawnRooms`). Empty for a map of a game without any.
pub fn spawn_room_zones(world: &World) -> Vec<MapBrush> {
    let (Some(game), Some(rooms), Some(entities)) = (
        world.get_resource::<crate::core::MapGame>(),
        world.get_resource::<SpawnRooms>(),
        world.get_resource::<super::MapEntities>(),
    ) else {
        return Vec::new();
    };
    let classes = rooms.of(&game.0);
    if classes.is_empty() {
        return Vec::new();
    }
    let s = entities.scale;
    entities
        .entities
        .iter()
        .filter(|e| classes.iter().any(|c| e.classname().eq_ignore_ascii_case(c)))
        .flat_map(|e| {
            let (rotation, origin) = (super::entities::entity_rotation(e.angles()), e.origin());
            e.hulls.iter().filter(|h| !h.points.is_empty()).map(move |h| {
                let planes = h
                    .planes
                    .iter()
                    .map(|(n, d)| {
                        let n = rotation * *n;
                        (super::entities::entity_to_engine(n, 1.0), (d + n.dot(origin)) * s)
                    })
                    .collect();
                let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
                for p in &h.points {
                    let q = super::entities::entity_to_engine(rotation * *p + origin, s);
                    (min, max) = (min.min(q), max.max(q));
                }
                MapBrush { planes, min, max, ladder: false, surface: None }
            })
        })
        .collect()
}
