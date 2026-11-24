//! The BSP 30 reader on the install's maps, against specs/goldsrc/bsp.md's
//! worked examples and test cases (de_dust2's lumps, textures, face 0's
//! lightmap, model 1's hulls, the spawns' clearance, visibility) and over
//! every map of `cstrike/` and `valve/` (each parses, its lightmaps follow
//! one another in face order from the lighting lump's start; three of
//! Half-Life's deathmatch maps carry a few hundred bytes after the last).

use goldsrc_formats::bsp::hull::{CONTENTS_SOLID, blocks};
use goldsrc_formats::bsp::{Bsp, Lump, wad_names};

use super::install::{files, mount};

fn dust2() -> Option<Bsp> {
    let m = mount()?;
    Some(Bsp::parse(&m.read("maps/de_dust2.bsp").unwrap()).unwrap())
}

#[test]
fn de_dust2_lumps_and_counts() {
    let Some(b) = dust2() else { return };
    assert_eq!(b.lumps[Lump::Planes as usize].offset, 124);
    assert_eq!(b.lumps[Lump::Planes as usize].len, 191_640);
    assert_eq!(b.lumps[Lump::Entities as usize].offset, 1_315_884);
    assert_eq!(
        (
            b.planes.len(),
            b.nodes.len(),
            b.clip_nodes.len(),
            b.leaves.len(),
            b.faces.len(),
            b.texinfo.len()
        ),
        (9_582, 2_766, 8_321, 1_455, 5_383, 638)
    );
    assert_eq!(
        (
            b.vertices.len(),
            b.edges.len(),
            b.surf_edges.len(),
            b.mark_surfaces.len()
        ),
        (6_555, 12_837, 24_472, 6_520)
    );
    assert_eq!(b.models.len(), 43);
    assert_eq!(b.lighting.len() % 3, 0);
    assert_eq!(b.leaves[0].contents, CONTENTS_SOLID);
    let m0 = &b.models[0];
    assert_eq!(
        (m0.mins, m0.maxs),
        ([-2816.0, -1664.0, -320.0], [2048.0, 3936.0, 640.0])
    );
    assert_eq!(
        (m0.heads, m0.vis_leaves, m0.face_count),
        ([0, 0, 2350, 4521], 1201, 5130)
    );
    assert_eq!(b.models[1].heads, [2513, 7358, 7364, 7370]);
}

#[test]
fn de_dust2_textures() {
    let Some(b) = dust2() else { return };
    assert_eq!(b.textures.len(), 44);
    assert_eq!(b.textures.iter().flatten().filter(|t| t.embedded()).count(), 15);
    let t0 = b.textures[0].as_ref().unwrap();
    assert_eq!(
        (t0.name.as_str(), t0.width, t0.height, t0.embedded()),
        ("+0~fifties_lgt2", 16, 64, false)
    );
    let t8 = b.textures[8].as_ref().unwrap();
    assert_eq!((t8.name.as_str(), t8.width, t8.height), ("-0SandRock2", 128, 256));
    assert_eq!(t8.mip_offsets, [40, 32_808, 41_000, 43_048]);
    assert_eq!((t8.colour_count, t8.palette.len()), (256, 256));
    assert_eq!(t8.rgba(0).unwrap().len(), 128 * 256 * 4);
}

#[test]
fn de_dust2_face_0_lightmap() {
    let Some(b) = dust2() else { return };
    let f = b.faces[0];
    assert_eq!(
        (f.plane, f.side, f.first_edge, f.edge_count, f.texinfo),
        (1776, 0, 0, 4, 8)
    );
    assert_eq!((f.styles, f.light_offset), ([0, 255, 255, 255], 0));
    let e = b.lightmap_extent(&f).unwrap();
    assert_eq!((e.s_range, e.t_range), ([-1952.0, -1856.0], [-64.0, -32.0]));
    assert_eq!(e.size, [7, 3]);
    assert_eq!(b.face_lightmap(&f).unwrap().len(), 63);
}

#[test]
fn de_dust2_hulls_entities_and_visibility() {
    let Some(b) = dust2() else { return };
    // Model 1 (a crate): hull 1's top at z = 100, hull 3's at 82.
    assert!(blocks(b.contents(1, 1, [-1400.0, 2620.0, 99.0])));
    assert!(!blocks(b.contents(1, 1, [-1400.0, 2620.0, 101.0])));
    assert!(blocks(b.contents(1, 3, [-1400.0, 2620.0, 81.0])));
    assert!(!blocks(b.contents(1, 3, [-1400.0, 2620.0, 83.0])));
    let ents = b.entities();
    let world = &ents[0];
    assert_eq!(world.classname(), "worldspawn");
    assert_eq!(wad_names(world), vec!["halflife.wad", "decals.wad", "cs_dust.wad"]);
    assert_eq!(
        (world.get("skyname"), world.get("MaxRange")),
        (Some("des"), Some("8192"))
    );
    // The usual spawns stand 4 units above hull 1's solid.
    let mut standing = 0;
    for e in ents.iter().filter(|e| e.classname() == "info_player_start") {
        let o: Vec<f64> = e
            .get("origin")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let below = |d: f64| blocks(b.contents(0, 1, [o[0], o[1], o[2] - d]));
        if below(4.5) && !below(4.0) {
            standing += 1;
        }
    }
    assert!(standing >= 18, "{standing} spawns stand 4 units above hull 1");
    assert_eq!(b.visible_leaves(1).unwrap().len(), 196);
}

#[test]
fn every_map_parses_and_its_lightmaps_follow_one_another() {
    let maps: Vec<_> = files("cstrike/maps", "bsp")
        .into_iter()
        .chain(files("valve/maps", "bsp"))
        .collect();
    if maps.is_empty() {
        return;
    }
    let (mut bad, mut trailing) = (Vec::new(), Vec::new());
    for path in &maps {
        let b = match Bsp::parse(&std::fs::read(path).unwrap()) {
            Ok(b) => b,
            Err(e) => {
                bad.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let mut end = 0usize;
        for f in b.faces.iter().filter(|f| f.light_offset >= 0) {
            let e = b.lightmap_extent(f).unwrap();
            if f.light_offset as usize != end {
                bad.push(format!(
                    "{}: a face's lightmap at {} where {end} was expected",
                    path.display(),
                    f.light_offset
                ));
                break;
            }
            end += (e.size[0] * e.size[1]) as usize * 3 * Bsp::style_count(f);
        }
        if end > b.lighting.len() {
            bad.push(format!(
                "{}: lightmaps end at {end}, past the lump's {}",
                path.display(),
                b.lighting.len()
            ));
        } else if end < b.lighting.len() {
            trailing.push(path.file_stem().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(bad.is_empty(), "{} maps; {}", maps.len(), bad.join("\n"));
    assert!(maps.len() >= 150, "{} maps", maps.len());
    // Bytes after the last lightmap: only these (specs/goldsrc/bsp.md says
    // none; reported to the spec).
    trailing.sort();
    assert!(
        trailing
            .iter()
            .all(|m| ["disposal", "pool_party", "rocket_frenzy"].contains(&m.as_str())),
        "{trailing:?}"
    );
}
