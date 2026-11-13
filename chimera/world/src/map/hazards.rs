//! Zones a player must not be placed in, registered per game and read by
//! whatever places players on a map (the minigames' random spawns,
//! `chimera_sim::minigame::spawns`): a function returning the world-space
//! brushes (engine space, m) that hurt, kill or drown a player standing
//! in them (CS:S's `trigger_hurt`, Combat Arms' damage and liquid
//! volumes). Water volumes (`core::MapWater`) count without registering.
//! Called when the zones are wanted, so a source sees the map's current
//! state. And per game, how high the spots' walker climbs on its maps
//! (`SpawnClimbs`).

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
