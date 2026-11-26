//! What CS 1.6's texture names say (specs/goldsrc/bsp.md, "Texture name
//! prefixes"; `goldsrc_engine::content::textures`), on the user's install:
//! cs_office's `+0~lab_crt2` animates its four frames in order at ten a
//! second and a button's `~elev2_pan` carries its alternate set; de_aztec's
//! `!watergreen` warps and nothing else does; de_dust2's `-nSandRock2`
//! variants all show, the same faces each conversion; the gsg9, leet and
//! sas bodies draw their chrome textures on generated coordinates
//! (mdl.md texture flag 2); every stock map's sky resolves and its
//! alpha-tested images' holes keep the opaque texels' colour, not the
//! blue key colour or black. Passes with a note without an install.

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
    assert_eq!(
        names(&map, &crt.frames),
        ["+0~lab_crt2", "+1~lab_crt2", "+2~lab_crt2", "+3~lab_crt2"]
    );
    assert_eq!(crt.rate, FRAME_RATE);
    // Its faces bind a frame of it (the renderer swaps the rest in).
    let mesh = map
        .meshes
        .iter()
        .find(|m| m.material.ends_with("~lab_crt2"))
        .expect("a ~lab_crt2 mesh");
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
    let toggled: Vec<Vec<String>> = map
        .meshes
        .iter()
        .filter(|m| !m.frames.is_empty())
        .map(|m| names(&map, &m.frames))
        .collect();
    eprintln!(
        "cs_office: {} animations, toggled meshes {toggled:?}",
        map.texture_anims.len()
    );
    assert!(!toggled.is_empty());
    for t in &toggled {
        assert!(
            t.len() == 2 && t[0].starts_with("+0") && t[1].starts_with("+a") && t[0][2..] == t[1][2..],
            "{t:?}"
        );
    }
}

#[test]
fn de_aztec_water_warps() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_aztec", false);
    let water: Vec<&str> = map
        .meshes
        .iter()
        .filter(|m| m.warp.is_some())
        .map(|m| m.material.as_str())
        .collect();
    assert!(
        !water.is_empty() && water.iter().all(|m| *m == "!watergreen"),
        "{water:?}"
    );
    assert!(
        map.meshes
            .iter()
            .filter(|m| m.material.starts_with('!'))
            .all(|m| m.warp.is_some())
    );
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

/// Every stock map's sky resolves and its alpha-tested images decode with
/// sane colours (specs/goldsrc/mount.md "Where things live", bsp.md
/// "Entities"; wad3.md "Converting to RGBA"): the six faces of
/// `gfx/env/<skyname>` (`desert` when unnamed) are found, decoded and not
/// flat; and in each image with holes (`{` textures, alpha-test sprites)
/// the holes carry the opaque texels' colour, not the key colour (blue in
/// every stock file) nor black, which filtering and the mip levels mix
/// into the leaves and blades beside them (de_aztec's grass sprites drew
/// blue-purple, its vines black).
#[test]
fn every_maps_sky_resolves_and_masked_images_keep_their_colours() {
    let Some(mount) = mount() else { return };
    let names = cs16::game::maps::map_names(&mount);
    assert!(names.len() >= 25, "{names:?}");
    let mut images = 0;
    for name in &names {
        let map = convert(&mount, name, true);
        let bsp =
            goldsrc_formats::bsp::Bsp::parse(&mount.read(&goldsrc_engine::content::maps::map_path(name)).unwrap())
                .unwrap();
        let entities = bsp.entities();
        let skyname = entities
            .first()
            .and_then(|w| w.get("skyname"))
            .unwrap_or("desert")
            .to_ascii_lowercase();
        let sky_warnings: Vec<&String> = map.warnings.iter().filter(|w| w.starts_with("sky")).collect();
        let sky = map
            .sky
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: no sky ({skyname}): {sky_warnings:?}"));
        assert!(sky_warnings.is_empty(), "{name}: {sky_warnings:?}");
        for (t, _) in sky.faces {
            let tex = &map.textures[t];
            assert!(
                tex.name.to_ascii_lowercase().starts_with(&format!("gfx/env/{skyname}")),
                "{name}: sky face {}",
                tex.name
            );
            assert!(
                tex.width > 0 && tex.rgba8.len() == (tex.width * tex.height * 4) as usize,
                "{name}: {}",
                tex.name
            );
            let lum: Vec<f32> = tex
                .rgba8
                .chunks(4)
                .map(|p| p[0] as f32 + p[1] as f32 + p[2] as f32)
                .collect();
            let mean = lum.iter().sum::<f32>() / lum.len() as f32;
            let spread = (lum.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / lum.len() as f32).sqrt();
            assert!(
                spread > 3.0 || mean < 3.0,
                "{name}: sky face {} is flat ({mean}, spread {spread})",
                tex.name
            );
        }
        for tex in map.textures.iter().filter(|t| !t.name.starts_with("gfx/env/")) {
            // The opaque texels beside a hole (what fills it) and the holes.
            let (w, h) = (tex.width as usize, tex.height as usize);
            let alpha = |x: usize, y: usize| tex.rgba8[(y * w + x) * 4 + 3];
            let (mut edge, mut holes) = (([0f64; 3], 0usize), ([0f64; 3], 0usize));
            for y in 0..h {
                for x in 0..w {
                    let a = alpha(x, y);
                    let acc = if a == 0 {
                        &mut holes
                    } else if a == 255
                        && (y.saturating_sub(1)..(y + 2).min(h))
                            .any(|ny| (x.saturating_sub(1)..(x + 2).min(w)).any(|nx| alpha(nx, ny) == 0))
                    {
                        &mut edge
                    } else {
                        continue;
                    };
                    let p = &tex.rgba8[(y * w + x) * 4..];
                    for c in 0..3 {
                        acc.0[c] += p[c] as f64;
                    }
                    acc.1 += 1;
                }
            }
            if edge.1 == 0 || holes.1 == 0 {
                continue;
            }
            images += 1;
            let mean = |(sum, n): ([f64; 3], usize)| sum.map(|s| s / n as f64);
            let (e, hl) = (mean(edge), mean(holes));
            let blue = |c: [f64; 3]| c[2] - c[0].max(c[1]);
            assert!(
                blue(hl) <= blue(e).max(0.0) + 24.0,
                "{name}: {}'s holes are bluer ({hl:?}) than the opaque texels around them ({e:?}): the key colour bleeds",
                tex.name
            );
            let bright = |c: [f64; 3]| c.iter().sum::<f64>();
            assert!(
                bright(hl) >= bright(e) * 0.5 - 6.0,
                "{name}: {}'s holes ({hl:?}) are much darker than the opaque texels around them ({e:?}): black bleeds",
                tex.name
            );
        }
    }
    eprintln!(
        "{} maps: every sky found; {images} images with holes keep their colours",
        names.len()
    );
}
