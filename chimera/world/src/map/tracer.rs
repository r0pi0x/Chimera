//! Bullet tracers for any game: a game asks for one with a `Tracer`
//! message (from the shot's start to its end, with the game's look); the
//! streak starts at the shooter's gun muzzle as last drawn (the view
//! model's for your own gun, re-projected as its muzzle flash is, else the
//! held world model's), else at the held weapon's muzzle
//! (`map::muzzle::HeldMuzzles`), the shot's start (the eye) only without one.
//!
//! Bullet tracers for any game: `Tracer` requests (the game's look and
//! frequency) and the shooters' last muzzle-flash points (`MuzzleCache`);
//! `render::tracer` draws them.

use std::collections::HashMap;

use bevy::prelude::*;


/// How a game's tracers look (meters, seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TracerLook {
    /// Particle material name.
    pub material: &'static str,
    pub speed: f32,
    /// Streak length and core half-width ranges.
    pub length: (f32, f32),
    pub half_width: (f32, f32),
    /// Shots shorter than this draw nothing.
    pub min_distance: f32,
}

/// Draw a tracer for `owner`'s shot from `from` to `to`.
#[derive(Message, Clone, Debug)]
pub struct Tracer {
    pub owner: Entity,
    pub from: Vec3,
    pub to: Vec3,
    pub look: TracerLook,
}

/// Each shooter's last muzzle-flash point (world) and when (seconds).
#[derive(Resource, Default)]
pub struct MuzzleCache(pub HashMap<Entity, (Vec3, f64)>);
