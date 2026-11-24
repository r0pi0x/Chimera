//! The scenario's vehicle placements as the engine's props and map
//! entities (specs/halo_ce/scenario.md vehicle placements;
//! vehicles.md "Multiplayer", "Which placements spawn"): a placement spawns
//! (`spawns`) when its palette entry is in range, its "not automatic" bit
//! (flags bit 0) is clear and the game type's vehicle set keeps its tag,
//! which must be one of the globals' three multiplayer vehicles
//! (`MULTIPLAYER_VEHICLES`); the on-BSP flags are scenery's alone and the
//! difficulty bits are ignored. Each one of a vehicle with a document in
//! data/vehicles/ (`VehicleDef::builtin`) becomes
//!
//! - a `MapProp` of the vehicle's render model, rigged (its nodes as bones,
//!   skinned meshes, the graph's vehicle overlays as its animation set:
//!   `look::anim_set`) and naming its map entity, so the drawn prop has
//!   joints `look::pose` moves; with `physics` set so the engine treats it
//!   as a moving body (its node under the map's root, replicated as a
//!   prop), and no collider of the engine's: `super` gives it its own; and
//! - a map entity of class `ENTITY_CLASS` naming the prop, the vehicle, the
//!   placement's position and axes, the model-space positions of the
//!   seats' markers (and the root node's, `camera::ROOT_NODE`), the
//!   collision model's vertices (the hull the vehicle's collider is built
//!   from), all in world units, and the graph's suspension elements.
//!
//! Halo's game does not model game types yet: `vehicles` places with the
//! default vehicle set (`VehicleSet::Default`, all three kinds); a game
//! type's set goes through `vehicles_in`. The placement's position is the
//! vehicle's centre of mass (`super::spawn_state` raises it).
//!
//! Whether a load places them at all (`placed_with`): always on Halo's
//! own gameplay (its app and server: the load reads the whole kit); in
//! another game's app (a party app's Halo map) only with the console
//! variable `halo_ce:party_vehicles` 1 (default 0; `party_vehicles` puts
//! it in the app's load context as `PartyVehicles`, which a listen
//! server's loads use too; the loader reads it when a map loads, so it applies from the next map: the first map of a launch with
//! `--map` is loaded before the console runs): off, no vehicle prop, entity
//! or model is in the map at all.
//!
//! Known shortcuts:
//! - The vehicle's collider is the convex hull of its collision model's
//!   vertices, not its surfaces (an open-topped Warthog is solid to its
//!   rim for players and shots).

use std::collections::BTreeMap;

use std::sync::Arc;

use blam_formats::{
    animation::AnimationGraph, cache::MapFile, collision, game_type::VehicleSet, model::Model, object::Vehicle,
    scenario::Scenario,
};

use super::data::VehicleDef;
use super::look::{self, Suspension};
use crate::game::body::skeleton;
use crate::game::maps::{
    objects::{self, Dest, Pending},
    space,
};
use crate::map::{LightProbe, MapData, MapEntity, MapModel, MapPhysics, MapProp, MapRig, PropSolid, PushAway};

/// The map entity of a placed vehicle.
pub const ENTITY_CLASS: &str = "halo_ce_vehicle";

/// A placed vehicle as its map entity carries it (world units, Halo axes).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// Index into `MapData::props`.
    pub prop: usize,
    /// The document's name.
    pub vehicle: String,
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
    /// Marker name to model-space position.
    pub markers: BTreeMap<String, [f32; 3]>,
    /// The collision model's vertices, model space.
    pub hull: Vec<[f32; 3]>,
    /// The graph's suspension elements (`look::suspension_key` names
    /// their overlays in the model's rig).
    pub suspension: Vec<Suspension>,
}

fn fmt3(p: [f32; 3]) -> String {
    format!("{},{},{}", p[0], p[1], p[2])
}

fn parse3(s: &str) -> Option<[f32; 3]> {
    let mut it = s.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

impl Placed {
    /// As a map entity.
    pub fn entity(&self) -> MapEntity {
        let markers: Vec<String> = self.markers.iter().map(|(n, p)| format!("{n}:{}", fmt3(*p))).collect();
        let hull: Vec<String> = self.hull.iter().map(|p| fmt3(*p)).collect();
        let suspension: Vec<String> = self
            .suspension
            .iter()
            .map(|s| format!("{},{},{}", s.point, s.extension, s.compression))
            .collect();
        MapEntity {
            keyvalues: vec![
                ("classname".into(), ENTITY_CLASS.into()),
                ("prop".into(), self.prop.to_string()),
                ("vehicle".into(), self.vehicle.clone()),
                ("position".into(), fmt3(self.position)),
                ("forward".into(), fmt3(self.forward)),
                ("up".into(), fmt3(self.up)),
                ("markers".into(), markers.join(";")),
                ("hull".into(), hull.join(";")),
                ("suspension".into(), suspension.join(";")),
            ],
            ..Default::default()
        }
    }

    /// Read back from a map entity of `ENTITY_CLASS`.
    pub fn from_entity(e: &MapEntity) -> Option<Placed> {
        if e.classname() != ENTITY_CLASS {
            return None;
        }
        let markers = e
            .get("markers")
            .unwrap_or("")
            .split(';')
            .filter(|s| !s.is_empty())
            .filter_map(|m| {
                let (n, p) = m.rsplit_once(':')?;
                Some((n.to_string(), parse3(p)?))
            })
            .collect();
        let hull = e
            .get("hull")
            .unwrap_or("")
            .split(';')
            .filter(|s| !s.is_empty())
            .filter_map(parse3)
            .collect();
        Some(Placed {
            prop: e.get("prop")?.parse().ok()?,
            vehicle: e.get("vehicle")?.to_string(),
            position: parse3(e.get("position")?)?,
            forward: parse3(e.get("forward")?)?,
            up: parse3(e.get("up")?)?,
            markers,
            hull,
            suspension: e
                .get("suspension")
                .unwrap_or("")
                .split(';')
                .filter_map(|s| {
                    let mut it = s.split(',');
                    Some(Suspension {
                        point: it.next()?.trim().parse().ok()?,
                        extension: it.next()?.trim().parse().ok()?,
                        compression: it.next()?.trim().parse().ok()?,
                    })
                })
                .collect(),
        })
    }
}

/// The globals' three multiplayer vehicles (globals.md multiplayer
/// information), in the vehicle sets' order: Warthog, Ghost, Scorpion.
pub const MULTIPLAYER_VEHICLES: [&str; 3] = [
    r"vehicles\warthog\warthog",
    r"vehicles\ghost\ghost_mp",
    r"vehicles\scorpion\scorpion_mp",
];

/// Whether a vehicle placement with placement flags `flags` naming the tag
/// `tag` (`None`: its palette index names no entry) is created in a
/// multiplayer game of vehicle set `set` (vehicles.md "Which placements
/// spawn"; the name rule is moot: the disc's placements have none).
pub fn spawns(flags: u16, tag: Option<&str>, set: VehicleSet) -> bool {
    let Some(tag) = tag else { return false };
    if flags & 1 != 0 {
        return false;
    }
    let Some(kind) = MULTIPLAYER_VEHICLES.iter().position(|t| t.eq_ignore_ascii_case(tag)) else {
        return false;
    };
    match set {
        VehicleSet::Default => true,
        VehicleSet::None => false,
        VehicleSet::Warthog => kind == 0,
        VehicleSet::Ghost => kind == 1,
        VehicleSet::Scorpion => kind == 2,
    }
}

/// Halo's vehicles wanted on Halo's maps in another game's app (module
/// doc): a value of the app's load context (`map::LoadContext`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartyVehicles(pub bool);

/// The console variable that sets `PartyVehicles` in a party app.
pub const PARTY_VEHICLES_CVAR: &str = "halo_ce:party_vehicles";

/// Whether a load with `options` places the vehicles (module doc): Halo's
/// own gameplay (the whole kit), or another game's app with
/// `PartyVehicles` on.
pub fn placed_with(options: &crate::map::MapLoadOptions) -> bool {
    options.kit.pickups()
        || options.context.get::<PartyVehicles>().is_some_and(|p| p.0)
}

/// The console variable (0 or 1) and what it sets: the app's load context
/// (`PartyVehicles`, the context a listen server's loads use too), at
/// once, so a map load right after it (the next command of the same line)
/// sees it. What
/// Halo's party bundle adds (`game::BUNDLE`'s `sim`).
pub fn party_vehicles(app: &mut bevy::prelude::App) {
    use chimera_core::console::ConsoleAppExt;
    app.console_cvar(
        PARTY_VEHICLES_CVAR,
        "1: Halo's vehicles are placed on its maps in this app (read when a map loads; default 0: none).",
        "0",
        |w| {
            let on = crate::map::LoadContext::of(w).get::<PartyVehicles>().is_some_and(|p| p.0);
            Some(if on { "1" } else { "0" }.to_string())
        },
        |w, v| {
            let on = match v.trim() {
                "0" => false,
                "1" => true,
                other => return Err(format!("bad value \"{other}\" (0 or 1)")),
            };
            crate::map::LoadContext::of(w).insert(PartyVehicles(on));
            Ok(())
        },
    );
}

/// Place the scenario's vehicles of `defs` in `map` with the default
/// vehicle set (module doc); returns the models' meshes waiting for their
/// looks.
pub fn vehicles(file: &MapFile, scenario: &Scenario, light: &LightProbe, map: &mut MapData) -> Vec<Pending> {
    vehicles_in(file, scenario, light, map, VehicleSet::Default)
}

/// `vehicles` under a game type's vehicle set.
pub fn vehicles_in(
    file: &MapFile,
    scenario: &Scenario,
    light: &LightProbe,
    map: &mut MapData,
    set: VehicleSet,
) -> Vec<Pending> {
    vehicles_of(file, scenario, light, map, &VehicleDef::builtin(), set)
}

/// What one vehicle tag gives every placement of it.
struct Kind {
    def: usize,
    model: usize,
    markers: BTreeMap<String, [f32; 3]>,
    hull: Vec<[f32; 3]>,
    suspension: Vec<Suspension>,
}

fn vehicles_of(
    file: &MapFile,
    scenario: &Scenario,
    light: &LightProbe,
    map: &mut MapData,
    defs: &[VehicleDef],
    set: VehicleSet,
) -> Vec<Pending> {
    let palette = &scenario.vehicle_palette;
    let mut pending = Vec::new();
    let mut kinds: BTreeMap<usize, Option<Kind>> = BTreeMap::new();
    for p in &scenario.vehicles {
        let h = &p.header;
        let pi = h.palette_entry(palette.len());
        let tag = pi.and_then(|i| file.tag(palette[i].object.id)).map(|t| t.path.as_str());
        let Some(pi) = pi.filter(|_| spawns(h.flags, tag, set)) else {
            continue;
        };
        let kind = kinds
            .entry(pi)
            .or_insert_with(|| kind_of(file, &palette[pi].object, map, defs, &mut pending));
        let Some(kind) = kind else { continue };
        let o = h.orientation();
        let prop = map.props.len();
        map.props.push(MapProp {
            model: kind.model,
            pose: None,
            ragdoll: None,
            translation: space::point(h.position),
            rotation: objects::placement_rotation(&o),
            solid: PropSolid::None,
            skybox: false,
            lighting: Some(light.clone()),
            vertex_light: None,
            casts_shadow: false,
            physics: Some(MapPhysics {
                mass: defs[kind.def].physics.mass,
                friction: 0.0,
                elasticity: 0.0,
                damping: 0.0,
                rotdamping: 0.0,
                inertia: None,
                push: PushAway::Collide,
                frozen: false,
                asleep: false,
                debris: false,
            }),
            fade: None,
            parent: None,
            // Its map entity (next): an entity prop gets its rig's joints
            // (`look::pose`).
            entity: Some(map.entities.len()),
            skin: 0,
            body: 0,
            detail: false,
        });
        let placed = Placed {
            prop,
            vehicle: defs[kind.def].name.clone(),
            position: h.position,
            forward: o.forward,
            up: o.up,
            markers: kind.markers.clone(),
            hull: kind.hull.clone(),
            suspension: kind.suspension.clone(),
        };
        map.entities.push(placed.entity());
    }
    pending
}

fn kind_of(
    file: &MapFile,
    r: &blam_formats::cache::TagRef,
    map: &mut MapData,
    defs: &[VehicleDef],
    pending: &mut Vec<Pending>,
) -> Option<Kind> {
    let tag = file.tag(r.id)?;
    let def = defs.iter().position(|d| d.tag.eq_ignore_ascii_case(&tag.path))?;
    let vehicle = match Vehicle::read(file, tag) {
        Ok(v) => v,
        Err(e) => {
            map.warnings
                .push(format!("vehicle `{}` unreadable, not placed: {e}", tag.path));
            return None;
        }
    };
    let model = match file.tag(vehicle.object.model.id).map(|t| Model::read(file, t)) {
        Some(Ok(m)) => m,
        Some(Err(e)) => {
            map.warnings
                .push(format!("vehicle `{}`: model unreadable, not placed: {e}", tag.path));
            return None;
        }
        None => {
            map.warnings
                .push(format!("vehicle `{}` has no model, not placed", tag.path));
            return None;
        }
    };
    let meshes = objects::skinned_meshes(file, &model, &|p| space::point(p));
    let index = map.models.len();
    // Its nodes as a rig and the graph's vehicle overlays (`look`).
    let bones = skeleton::bones(&model);
    let (set, suspension) = match file.tag(vehicle.object.animation_graph.id).map(|t| AnimationGraph::read(file, t)) {
        Some(Ok(graph)) => {
            let (set, suspension, left_out) = look::anim_set(&graph, &bones, model.node_list_checksum);
            map.warnings.extend(left_out);
            (set, suspension)
        }
        Some(Err(e)) => {
            map.warnings
                .push(format!("vehicle `{}`: animation graph unreadable, drawn still: {e}", tag.path));
            (look::anim_set_still(&bones), Vec::new())
        }
        None => (look::anim_set_still(&bones), Vec::new()),
    };
    let mut lo = bevy::math::Vec3::splat(f32::MAX);
    let mut hi = bevy::math::Vec3::splat(f32::MIN);
    for (m, _) in &meshes {
        for p in &m.positions {
            lo = lo.min((*p).into());
            hi = hi.max((*p).into());
        }
    }
    map.models.push(MapModel {
        bounds: if lo.x <= hi.x { (lo, hi) } else { Default::default() },
        rig: Some(Arc::new(MapRig {
            bones,
            root: skeleton::root(),
            animations: Arc::new(set),
        })),
        ..Default::default()
    });
    pending.extend(meshes.into_iter().map(|(mesh, shader)| Pending {
        mesh,
        shader,
        lit: true,
        dest: Dest::Model(index),
    }));
    let bind = model.bind_pose();
    let mut markers = BTreeMap::new();
    let wanted: Vec<&str> = defs[def]
        .seats
        .iter()
        .flat_map(|s| [s.marker.as_str(), s.camera_marker.as_str()])
        .filter(|n| !n.is_empty())
        .collect();
    for m in &model.markers {
        if wanted.iter().any(|w| w.eq_ignore_ascii_case(&m.name))
            && let Some(at) = m.instances.first().and_then(|i| model.marker_world(i, &bind))
        {
            markers.insert(m.name.to_lowercase(), at.position);
        }
    }
    // The root node's place: the camera's focus when a seat's named camera
    // marker is missing (the Ghost's).
    if let Some(root) = bind.first() {
        markers.insert(super::camera::ROOT_NODE.to_string(), root.apply_point([0.0; 3]));
    }
    let mut hull = Vec::new();
    if let Some(ct) = file.tag(vehicle.object.collision_model.id)
        && let Ok(c) = collision::read_model(file, ct)
    {
        for (ni, node) in c.nodes.iter().enumerate() {
            let Some(bsp) = node.bsps.first() else { continue };
            let t = bind
                .get(ni)
                .copied()
                .unwrap_or(blam_formats::model::Transform::IDENTITY);
            hull.extend(bsp.vertices.iter().map(|v| t.apply_point(v.point)));
        }
    }
    Some(Kind {
        def,
        model: index,
        markers,
        hull,
        suspension,
    })
}
