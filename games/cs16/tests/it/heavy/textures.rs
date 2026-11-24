//! What CS 1.6's texture names say (specs/goldsrc/bsp.md, "Texture name
//! prefixes"; `goldsrc_engine::content::textures`), on the user's install:
//! cs_office's `+0~lab_crt2` animates its four frames in order at ten a
//! second and a button's `~elev2_pan` carries its alternate set; de_aztec's
//! `!watergreen` warps and nothing else does; de_dust2's `-nSandRock2`
//! variants all show, the same faces each conversion; the gsg9, leet and
//! sas bodies draw their chrome textures on generated coordinates
//! (mdl.md texture flag 2). Passes with a note without an install.

use chimera_world::map::{LayerCoords, MapData};
use cs16::game::bodies;
use goldsrc_engine::content::textures::FRAME_RATE;

use super::maps::{convert, mount};

/// The names of texture indices `frames`.
fn names(map: &MapData, frames: &[usize]) -> Vec<String> {
    frames.iter().map(|&i| map.textures[i].name.clone()).collect()
}

#[test]
fn cs_office_animates_its_frames_in_order() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "cs_office", false);
    let crt = map
        .texture_anims
        .iter()
        .find(|a| map.textures[a.frames[0]].name == "+0~lab_crt2")
        .unwrap_or_else(|| panic!("no +0~lab_crt2 animation: {:?}", map.texture_anims));
    assert_eq!(names(&map, &crt.frames), ["+0~lab_crt2", "+1~lab_crt2", "+2~lab_crt2", "+3~lab_crt2"]);
    assert_eq!(crt.rate, FRAME_RATE);
    // Its faces bind a frame of it (the renderer swaps the rest in).
    let mesh = map.meshes.iter().find(|m| m.material.ends_with("~lab_crt2")).expect("a ~lab_crt2 mesh");
    assert!(crt.frames.contains(&mesh.texture.unwrap()));
    // Every animation's frames are one family's, in order, and no frame is
    // in two animations.
    let mut seen = std::collections::HashSet::new();
    for a in &map.texture_anims {
        let n = names(&map, &a.frames);
        assert!(a.frames.len() >= 2 && a.frames.iter().all(|f| seen.insert(*f)), "{n:?}");
        let base = &n[0][2..];
        assert!(n.iter().all(|x| &x[2..] == base && x.starts_with('+')), "{n:?}");
    }
    // A mover with an alternate set: its first frames, off then on.
    let toggled: Vec<Vec<String>> =
        map.meshes.iter().filter(|m| !m.frames.is_empty()).map(|m| names(&map, &m.frames)).collect();
    eprintln!("cs_office: {} animations, toggled meshes {toggled:?}", map.texture_anims.len());
    assert!(!toggled.is_empty());
    for t in &toggled {
        assert!(t.len() == 2 && t[0].starts_with("+0") && t[1].starts_with("+a") && t[0][2..] == t[1][2..], "{t:?}");
    }
}

#[test]
fn de_aztec_water_warps() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_aztec", false);
    let water: Vec<&str> = map.meshes.iter().filter(|m| m.warp.is_some()).map(|m| m.material.as_str()).collect();
    assert!(!water.is_empty() && water.iter().all(|m| *m == "!watergreen"), "{water:?}");
    assert!(map.meshes.iter().filter(|m| m.material.starts_with('!')).all(|m| m.warp.is_some()));
}

#[test]
fn de_dust2_tiles_its_variants_at_random_the_same_each_time() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let shown = |map: &MapData, base: &str| -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = map
            .meshes
            .iter()
            .filter(|m| m.material.len() > 2 && m.material.starts_with('-') && &m.material[2..] == base)
            .map(|m| (m.material.clone(), m.indices.len()))
            .collect();
        v.sort();
        v
    };
    let rock = shown(&map, "sandrock2");
    let materials: Vec<&str> = rock.iter().map(|(m, _)| m.as_str()).collect();
    for v in ["-0sandrock2", "-1sandrock2", "-2sandrock2"] {
        assert!(materials.contains(&v), "{v} not shown: {rock:?}");
    }
    let again = convert(&mount, "de_dust2", false);
    assert_eq!(rock, shown(&again, "sandrock2"));
    assert_eq!(shown(&map, "sand"), shown(&again, "sand"));
}

#[test]
fn chrome_textures_draw_on_generated_coordinates() {
    let Some(mount) = mount() else { return };
    let mut map = MapData::default();
    bodies::add_bodies(&mount, false, &mut map);
    let mut chrome = 0;
    for c in &map.kit.characters {
        for m in &c.model.meshes {
            if let Some(l) = &m.layer {
                assert_eq!(l.coords, LayerCoords::SphereMap);
                assert_eq!(l.texture, m.texture, "{}", m.material);
                chrome += 1;
            }
        }
    }
    eprintln!("{chrome} chrome meshes");
    assert!(chrome > 0, "no chrome meshes on the eight bodies");
}
