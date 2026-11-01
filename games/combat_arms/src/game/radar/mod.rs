//! The radar and the full-screen map's data and rules (specs/combat_arms/
//! radar_minimap.md), with no drawing: the front end (`ui::hud::radar`,
//! `ui::hud::full_map`) reads these and draws.
//!
//! - `maths`: the spec's arithmetic on file coordinates and reference
//!   pixels (corners from the mission data, world to radar pixel, the
//!   rotation, wedges, the guide, edge arrows, the image window, the full
//!   map's transform, zoom);
//! - `look`: who is shown to whom and with which icon (R6, R9, F4, F5);
//! - `mission`: the per-map image data in the mission files (R1);
//! - `scan`: the scanner items and what they reveal (S1 to S3).
//!
//! What the server sends (N): at each round start `CaRadarRound`, the
//! current objective's image name and corners from the mission data
//! (`send_round`, the install's mission file for the world and mode, else
//! nothing) and, per team, the mean of its start points (F5, not in One
//! Man Army and Arms Race); it travels on the match's entity
//! (`game::net::NetCaRadar`) and a client keeps it as its own resource.
//! Everything else (which objects are drawn, transforms, zoom, rotation,
//! edge arrows, death marks, a player's own heartbeat contacts) a client
//! works out from ordinary objects.
//!
//! The options live here as the player's profile (R10): `RadarScale`
//! (saved when the radar hides) and `RotateRadar`, console variables
//! (`radarscale`, `rotateradar`) that the options' profile keeps
//! (`game::options`: `[Display] RadarScale`, `[Controls] RotateRadar`). The world offset
//! the map data adds (R5, F3) is the world file's, kept by `game::world`
//! as a pass-through entity (`WORLD_INFO`) and read into `CaWorldOffset`.
//!
//! Known shortcuts (docs/tech-debt.md; the spec's most literal reading):
//! - The round's data is sent when the match enters the setup (or the live
//!   phase when there is none) of a round, once per round and map load.
//!   The mission file is found by the world's name and the mode's suffix
//!   (`mission::find`: the files `gms/*_<suffix>.gms` whose `[MapInfo]`
//!   names the world), objective 0; a server without the install (or its V1
//!   key) sends no image, and a client does not read one itself.
//! - The start points' mean is of the list the mode spawns from
//!   (`rules::spawns::spawn_list`: unlocked, the mode's family, VIP
//!   Escort's swap), in the file's X and Z.
//! - Nothing drives the co-op objects (NPCs, supply points, mission
//!   targets and weapons): the modes aren't built.

pub mod legend;
pub mod look;
pub mod maths;
pub mod mission;
pub mod scan;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub use self::{
    maths::{Corners, Frame},
    mission::MapImage,
    scan::{CaScanner, CaScans, CaSeen},
};
use super::{
    inventory::UNIT,
    rules::{self, CaMatch, objects::CaMapObjects, state::Phase},
};
use crate::{
    console::resource_cvar,
    core::{MapLoads, NetRole, SimSet},
    map::{MapEntities, MapEntity},
    weapon::data::WeaponDataAppExt,
};

/// The pass-through entity class `game::world` adds for the world file's
/// properties the radar needs (`WorldOffsetX`, `WorldOffsetZ`, in the
/// file's units).
pub const WORLD_INFO: &str = "WorldInfo";

/// The player's radar options (R10), saved in the config.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaRadarProfile {
    /// `RadarScale`, the zoom (0.5 to 2.0, default 1).
    pub scale: f32,
    /// `RotateRadar`: 0 off (default), 1 on. Read when a world is entered.
    pub rotate: i32,
}

impl Default for CaRadarProfile {
    fn default() -> Self {
        Self {
            scale: maths::ZOOM_DEFAULT,
            rotate: 0,
        }
    }
}

/// The world file's offset, X and Z, for the map data (R5, F3).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct CaWorldOffset(pub Vec2);

/// What the server sends at a round start (N).
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaRadarRound {
    /// The current objective's map image and corners.
    pub image: MapImage,
    /// Alpha's and Bravo's spawn markers (file X, Z); None: no marker (no
    /// start points, or a mode without them).
    pub spawns: [Option<[f32; 2]>; 2],
}

/// Where the server gets a world's map image data: its world name and the
/// mode's room key. The install's mission files by default.
#[derive(Resource, Clone, Copy)]
pub struct RadarImageSource(pub fn(&str, &str) -> Option<MapImage>);

impl RadarImageSource {
    /// The install's mission files, unless `data` reads nothing of the
    /// install.
    pub fn for_data(data: &super::data::CaDataInner) -> Self {
        Self(if data.builtin_only() { |_, _| None } else { install_image })
    }
}

fn install_image(map: &str, mode: &str) -> Option<MapImage> {
    let mount = super::INFO.mount()?;
    mission::image_of(&mount, map, mode)
}

/// Engine metres as file units, rounded to a thousandth of a unit: the
/// engine's `f32` metres don't hold a round number of units exactly, and a
/// pixel is a truncation (R4's note on points at an integer boundary).
fn units(metres: f32) -> f64 {
    (metres as f64 / UNIT as f64 * 1000.0).round() / 1000.0
}

/// A file point (X, Z) of an engine position.
pub fn file_point(p: Vec3) -> (f64, f64) {
    (units(p.x), -units(p.z))
}

/// An engine position's height in file units.
pub fn file_height(p: Vec3) -> f64 {
    units(p.y)
}

/// The spec's yaw (from north, turning right) of an Intent yaw (engine:
/// 0 looks down -Z, turning left grows).
pub fn spec_yaw(intent_yaw: f32) -> f64 {
    -(intent_yaw as f64)
}

/// The room keys of the modes that send no spawn markers (F5).
const NO_MARKERS: &[&str] = &["FFA", "VWRM"];

/// The scanner weapons' hook alone (`scan::SCANNER`), for an app that
/// offers Combat Arms' weapons without its radar (`CaRadarPlugin` adds it).
pub struct CaScannerHook;

impl Plugin for CaScannerHook {
    fn build(&self, app: &mut App) {
        app.register_weapon_hook::<scan::ScannerParams>(scan::SCANNER, |e, _, p| {
            e.insert(scan::CaScannerData(*p));
        });
    }
}

pub struct CaRadarPlugin;

impl Plugin for CaRadarPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<RadarImageSource>() {
            let source = RadarImageSource::for_data(&super::data::CaData::of(app));
            app.insert_resource(source);
        }
        app.init_resource::<CaRadarProfile>()
            .init_resource::<CaWorldOffset>()
            .init_resource::<CaRadarRound>()
            .init_resource::<CaScans>()
            .init_resource::<scan::CaDetectors>()
            .add_plugins(CaScannerHook)
            .add_systems(
                FixedUpdate,
                (scan::scanners, scan::share, scan::mark)
                    .chain()
                    .in_set(SimSet::Weapons)
                    .run_if(crate::core::authoritative),
            )
            .add_systems(FixedLast, send_round.run_if(not(resource_equals(NetRole::Client))))
            .add_systems(Update, read_world_offset);
        resource_cvar(app, "combat_arms:radarscale", "The radar's zoom, 0.5 to 2.0 (saved when it hides).", |r: &mut CaRadarProfile| {
            &mut r.scale
        });
        resource_cvar(app, "combat_arms:rotateradar", "1: the radar turns with the view (read when a map loads).", |r: &mut CaRadarProfile| {
            &mut r.rotate
        });
    }
}

/// The world file's offset from the map's pass-through entity (zero
/// without one).
fn read_world_offset(entities: Option<Res<MapEntities>>, mut offset: ResMut<CaWorldOffset>) {
    let Some(m) = entities else { return };
    if !m.is_changed() {
        return;
    }
    let next = world_offset(&m.entities);
    if offset.0 != next {
        offset.0 = next;
    }
}

/// The offset among a map's entities.
pub fn world_offset(entities: &[MapEntity]) -> Vec2 {
    entities
        .iter()
        .find(|e| e.classname() == WORLD_INFO)
        .map(|e| {
            let n = |k: &str| e.get(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
            Vec2::new(n("WorldOffsetX"), n("WorldOffsetZ"))
        })
        .unwrap_or_default()
}

/// The mean of a team's start points in file X, Z (F5); None when it has
/// none (the server's division by zero: NaN, no marker).
pub fn spawn_mean(points: &[Transform]) -> Option<[f32; 2]> {
    if points.is_empty() {
        return None;
    }
    let n = points.len() as f64;
    let (x, z) = points.iter().map(|p| file_point(p.translation)).fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    Some([(x / n) as f32, (z / n) as f32])
}

/// N: the round's data at a round start: when the match enters a round's
/// setup (or live phase) once per map load and round.
fn send_round(world: &mut World, mut last: Local<Option<(u32, u32, bool, String)>>, mut cache: Local<Option<(String, String, MapImage)>>) {
    let (phase, round, warmup) = {
        let m = world.resource::<CaMatch>();
        (m.phase, m.round, m.warmup)
    };
    if !matches!(phase, Phase::Setup { .. } | Phase::Live) {
        return;
    }
    let loads = world.get_resource::<MapLoads>().map_or(0, |l| l.0);
    let mode = rules::current_mode(world).map(|m| m.key).unwrap_or("");
    let key = (loads, round, warmup, mode.to_string());
    if last.as_ref() == Some(&key) {
        return;
    }
    *last = Some(key);
    let map = world
        .get_resource::<crate::map::MapFile>()
        .map(|m| m.name.clone())
        .or_else(|| world.get_resource::<crate::map::LoadedMapName>().map(|m| m.0.clone()))
        .unwrap_or_default();
    let map = map.rsplit(':').next().unwrap_or("").to_string();
    let image = match cache.as_ref() {
        Some((m, k, image)) if *m == map && k == mode => image.clone(),
        _ => {
            let source = world.resource::<RadarImageSource>().0;
            let image = source(&map, mode).unwrap_or_default();
            *cache = Some((map.clone(), mode.to_string(), image.clone()));
            image
        }
    };
    let mut spawns = [None, None];
    if !mode.is_empty() && !NO_MARKERS.contains(&mode) && world.contains_resource::<CaMapObjects>() {
        for (i, team) in [rules::state::ALPHA, rules::state::BRAVO].into_iter().enumerate() {
            let (list, _) = rules::spawns::spawn_list(world, team);
            spawns[i] = spawn_mean(&list);
        }
    }
    let next = CaRadarRound { image, spawns };
    if *world.resource::<CaRadarRound>() != next {
        world.insert_resource(next);
    }
}
