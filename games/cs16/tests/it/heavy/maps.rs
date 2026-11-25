//! CS 1.6's maps converted (`game::maps`, `goldsrc_engine::content`) on
//! the user's install: de_dust2 converts with every texture found, its
//! spawns stand on the floor, a player walking from them never ends up
//! inside a wall or player clip (hull 1, the game's own player hull, is
//! the judge: docs/plans/active/goldsrc.md §3), and shots pass the player
//! clip that stops characters; cs_militia's switchable lights convert as
//! light styles, de_aztec's sprites and sounds as its entities name them.
//! Passes with a note without an install.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_core::core::{CHARACTER_FILTER, SOLID_LAYERS};
use chimera_world::{
    harness::Sim,
    map::{KitLoad, MapData, MapLoadOptions, MapSimPlugin},
};
use cs16::{
    game::{INFO, maps, movement},
    harness::Cs16Sim,
};
use goldsrc_engine::content::{entities::STAND_HALF_HEIGHT, maps as gmaps, to_game};
use goldsrc_formats::bsp::{Bsp, hull::blocks};

/// The install's mount, or None (with a note).
pub fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = INFO.install() else {
        eprintln!("no Counter-Strike 1.6 install (~/games/cs16, Steam, config or CHIMERA_CS16_ROOT): skipped");
        return None;
    };
    Some(cs16::game::mount::open_install(&install).unwrap_or_else(|e| panic!("{}: {e}", install.display())))
}

/// Map `name` converted (no cache), without its kit, textures decoded or not.
pub fn convert(mount: &chimera_core::mount::Mount, name: &str, textures: bool) -> MapData {
    let options = MapLoadOptions { textures, kit: KitLoad::Only(&[]), ..Default::default() };
    gmaps::load_in(&maps::CS16, mount, name, &options, None).unwrap_or_else(|e| panic!("{e}")).0
}

fn bsp(mount: &chimera_core::mount::Mount, name: &str) -> Bsp {
    Bsp::parse(&mount.read(&gmaps::map_path(name)).unwrap()).unwrap()
}

/// The transform above the feet, metres (the standing box's centre: 36
/// units).
const CENTRE: f32 = 36.0 * 0.0254;

#[test]
fn de_dust2_converts_with_every_texture() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", true);
    let missing: Vec<&String> = map.warnings.iter().filter(|w| w.starts_with("missing texture")).collect();
    assert!(missing.is_empty(), "{missing:?}");
    assert!(map.meshes.len() > 20, "{} meshes", map.meshes.len());
    assert!(map.meshes.iter().all(|m| m.lightmap_uvs.len() == m.positions.len()));
    assert!(map.lightmap.is_some() && map.sky.is_some(), "{:?}", map.warnings);
    assert!(map.textures.iter().all(|t| t.rgba8.len() == (t.width * t.height * 4) as usize));
    assert!(map.spawns.iter().filter(|(_, t)| t.map(|t| t.0) == Some(1)).count() >= 10);
    assert!(map.spawns.iter().filter(|(_, t)| t.map(|t| t.0) == Some(2)).count() >= 10);
    assert!(!map.player_clips.is_empty() && !map.collision_hulls.is_empty());
    eprintln!(
        "de_dust2: {} meshes, {} textures, {} solids, {} clip pieces, {} entities, warnings {:?}",
        map.meshes.len(),
        map.textures.len(),
        map.collision_hulls.len(),
        map.player_clips.len(),
        map.entities.len(),
        map.warnings
    );
    // The same bytes each time (the cache and served entries rely on it).
    let again = convert(&mount, "de_dust2", true);
    assert_eq!(map.meshes.len(), again.meshes.len());
    assert_eq!(map.collision_brushes.len(), again.collision_brushes.len());
    assert_eq!(map.lightmap.as_ref().map(|l| l.rgb.len()), again.lightmap.as_ref().map(|l| l.rgb.len()));
}

#[test]
fn de_dust2_spawns_stand_on_the_floor() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let b = bsp(&mount, "de_dust2");
    for (feet, _) in &map.spawns {
        let g = to_game(*feet);
        let centre = [g[0] as f64, g[1] as f64, (g[2] + STAND_HALF_HEIGHT) as f64];
        assert!(!blocks(b.contents(0, 1, centre)), "spawn {g:?}: the box is in solid");
        assert!(
            blocks(b.contents(0, 1, [centre[0], centre[1], centre[2] - 1.0])),
            "spawn {g:?}: not on the floor (free a unit below)"
        );
    }
    // A player put there comes to rest on it.
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).step_by(5).collect();
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map));
    sim.ticks(3);
    for feet in spawns {
        let p = sim.spawn_character(feet + Vec3::Y * (CENTRE + 0.05), movement::ID);
        sim.seconds(1.5);
        let at = sim.position(p);
        assert!(sim.state(p).on_ground, "spawn {feet}: not on the ground at {at}");
        assert!((at.y - CENTRE - feet.y).abs() < 0.1, "spawn {feet}: feet at {}", at.y - CENTRE);
        assert!((at.xz() - feet.xz()).length() < 0.3, "spawn {feet}: slid to {at}");
        sim.app.world_mut().despawn(p);
    }
}

#[test]
fn a_player_never_walks_into_walls_or_player_clip() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let b = bsp(&mount, "de_dust2");
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).step_by(4).collect();
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map));
    sim.ticks(3);
    // A capsule's centre up to 4 units from free space in hull 1 is
    // touching, not inside: the capsule is a quarter unit narrower than the
    // box and its centre half a unit lower, and its round bottom sinks a
    // few units over a step's edge that the box would rest on.
    let inside = |p: Vec3| {
        let g = to_game(p);
        let c = [g[0] as f64, g[1] as f64, g[2] as f64 + 0.6];
        let free = |d: [f64; 3]| !blocks(b.contents(0, 1, [c[0] + d[0], c[1] + d[1], c[2] + d[2]]));
        let mut offsets = vec![[0.0; 3]];
        for r in [1.0, 2.0, 4.0] {
            for d in [[r, 0.0, 0.0], [-r, 0.0, 0.0], [0.0, r, 0.0], [0.0, -r, 0.0], [0.0, 0.0, r], [0.0, 0.0, -r]] {
                offsets.push(d);
            }
        }
        !offsets.iter().any(|d| free(*d))
    };
    let mut faults = Vec::new();
    let mut walks = 0;
    for feet in spawns {
        for k in 0..8 {
            let p = sim.spawn_character(feet + Vec3::Y * (CENTRE + 0.05), movement::ID);
            sim.ticks(5);
            sim.intent(p).yaw = k as f32 * std::f32::consts::FRAC_PI_4;
            sim.intent(p).move_axis = Vec2::Y;
            for _ in 0..12 {
                sim.seconds(0.25);
                let at = sim.position(p);
                if inside(at) {
                    faults.push(format!("from {feet} heading {k}: inside hull 1's solid at {:?}", to_game(at)));
                    break;
                }
            }
            walks += 1;
            sim.app.world_mut().despawn(p);
        }
    }
    eprintln!("{walks} walks, {} faults", faults.len());
    assert!(faults.is_empty(), "{faults:#?}");
}

#[test]
fn shots_pass_player_clip_that_stops_characters() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let b = bsp(&mount, "de_dust2");
    // Centres of clip pieces in hull 0's open space (clip only).
    let probes: Vec<Vec3> = map
        .player_clips
        .iter()
        .filter_map(|&i| {
            let brush = &map.collision_brushes[i];
            let c = (brush.min + brush.max) / 2.0;
            let g = to_game(c);
            let within = |b: &chimera_core::core::MapBrush| b.planes.iter().all(|(n, d)| n.dot(c) <= *d - 0.01);
            // Inside its piece, in or touching no solid (the world's or a
            // brush entity's).
            let near = |b: &chimera_core::core::MapBrush| b.planes.iter().all(|(n, d)| n.dot(c) <= *d + 0.02);
            let solid = map.collision_brushes.iter().enumerate().any(|(j, s)| !map.trace_skip.contains(&j) && near(s));
            let gv = Vec3::from(g);
            let mover = map.entities.iter().filter(|e| e.mover).any(|e| {
                let o = e.origin();
                e.hulls.iter().any(|h| h.planes.iter().all(|(n, d)| n.dot(gv - o) <= *d))
            });
            (within(brush) && !solid && !mover && !blocks(b.contents(0, 0, [g[0] as f64, g[1] as f64, g[2] as f64]))).then_some(c)
        })
        .take(20)
        .collect();
    assert!(probes.len() >= 5, "only {} clip-only probes", probes.len());
    let mut sim = Sim::new(Cs16Sim, MapSimPlugin::new(map));
    sim.ticks(3);
    let hits = sim
        .app
        .world_mut()
        .run_system_once(move |q: SpatialQuery, names: Query<&Name>| {
            probes
                .iter()
                .map(|p| {
                    let shots = q.point_intersections(*p, &SpatialQueryFilter::from_mask(SOLID_LAYERS));
                    for e in &shots {
                        eprintln!("probe {p} ({:?}) meets {:?}", to_game(*p), names.get(*e));
                    }
                    (q.point_intersections(*p, &SpatialQueryFilter::from_mask(CHARACTER_FILTER)).len(), shots.len())
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    for (i, (characters, shots)) in hits.iter().enumerate() {
        assert!(*characters > 0, "probe {i}: characters pass the clip");
        assert_eq!(*shots, 0, "probe {i}: shots meet the clip");
    }
}

/// cs_militia's switchable lights (`shack`, style 33; `slight`, 32: both
/// start lit) carry their own lightmap shares beside style 0, which a
/// switch takes away; de_aztec draws its sprites (the unnamed ones on
/// from the start) and both play sounds the map's entities name.
#[test]
fn light_styles_sprites_and_sounds() {
    let Some(mount) = mount() else { return };
    let militia = convert(&mount, "cs_militia", false);
    let lightmap = militia.lightmap.as_ref().unwrap();
    let styles: Vec<(u8, bool)> = lightmap.styles.iter().map(|s| (s.style, s.on)).collect();
    assert_eq!(styles, vec![(32, true), (33, true)]);
    for s in &lightmap.styles {
        assert!(!s.texels.is_empty() && s.texels.len() == s.rgb.len() && s.pattern.is_empty());
        assert!(s.rgb.iter().any(|c| c.iter().any(|v| *v > 0.0)), "style {} adds light", s.style);
    }
    // Off, the shack's style takes its share away.
    let (dark, _) = lightmap.relit(&|s| if s.style == 33 { 0.0 } else { s.start() });
    let t = lightmap.styles[1].texels[lightmap.styles[1].rgb.iter().position(|c| c[0] > 0.05).unwrap()] as usize;
    assert!(dark[t][0] < lightmap.rgb[t][0] - 0.04);
    let aztec = convert(&mount, "de_aztec", false);
    assert_eq!(aztec.sprites.len(), 76, "{:?}", aztec.warnings);
    assert!(aztec.sprites.iter().all(|s| s.entity.is_some() && s.size.x > 0.0));
    assert!(aztec.sprites.iter().any(|s| s.start_on));
    // Its thunder clap (entities.md's worked example).
    assert!(aztec.sounds.entry("ambience/thunder_clap.wav").is_some(), "{:?}", aztec.sounds.entries.keys().collect::<Vec<_>>());
}
