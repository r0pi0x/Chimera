//! The drawn light on the install's maps (specs/goldsrc/bsp.md, "Drawn
//! light": test cases; mdl.md, "Lighting": cs_militia's sky colour): the
//! curve on the vines of de_aztec and the hostage room of cs_militia.
//! Passes with a note without an install.

use chimera_world::map::LightCurve;
use goldsrc_engine::content::model_light::sky_of;
use goldsrc_formats::bsp::{Bsp, texture_name};

use super::maps::{convert, mount};

fn bsp(mount: &chimera_core::mount::Mount, name: &str) -> Bsp {
    Bsp::parse(&mount.read(&goldsrc_engine::content::maps::map_path(name)).unwrap()).unwrap()
}

/// Face `i`'s texture name and its style 0 samples as (R, G, B) bytes.
fn face(bsp: &Bsp, i: usize) -> (String, Vec<[u8; 3]>) {
    let f = &bsp.faces[i];
    let name = texture_name(bsp, &bsp.texinfo[f.texinfo as usize]).unwrap_or("").to_string();
    let ext = bsp.lightmap_extent(f).unwrap();
    let n = (ext.size[0] * ext.size[1]) as usize;
    let bytes = bsp.face_lightmap(f).unwrap();
    assert_eq!(f.styles[0], 0, "face {i} style 0 first");
    (name, bytes[..n * 3].chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
}

fn lit(c: &LightCurve, s: [u8; 3]) -> [u32; 3] {
    s.map(|v| c.level(v as f32))
}

#[test]
fn the_vines_and_the_hostage_room_are_drawn_as_the_spec_says() {
    let Some(mount) = mount() else { return };
    let c = LightCurve::default();

    let aztec = bsp(&mount, "de_aztec");
    let (name, s) = face(&aztec, 5450);
    assert!(name.eq_ignore_ascii_case("{vine1"), "{name}");
    assert_eq!(s.len(), 15 * 9);
    let (lo, hi) = (s.iter().map(|p| p[0]).min().unwrap(), s.iter().map(|p| p[0]).max().unwrap());
    assert_eq!((lo, hi), (37, 51));
    let l: Vec<u32> = s.iter().map(|p| lit(&c, *p)[0]).collect();
    assert_eq!((*l.iter().min().unwrap(), *l.iter().max().unwrap()), (72, 99));
    assert_eq!(c.level(45.0), 88);
    let (name, s) = face(&aztec, 5457);
    assert!(name.eq_ignore_ascii_case("{vine1"), "{name}");
    let (lo, hi) = (s.iter().map(|p| p[0]).min().unwrap(), s.iter().map(|p| p[0]).max().unwrap());
    assert_eq!((lo, hi), (18, 28));
    let l: Vec<u32> = s.iter().map(|p| lit(&c, *p)[0]).collect();
    assert_eq!((*l.iter().min().unwrap(), *l.iter().max().unwrap()), (35, 54));
    assert_eq!(c.level(23.0), 45);

    let militia = bsp(&mount, "cs_militia");
    let (name, s) = face(&militia, 2350);
    assert!(name.eq_ignore_ascii_case("crete4_flr02"), "{name}");
    assert_eq!(s.len(), 4 * 15);
    let min = [0, 1, 2].map(|k| s.iter().map(|p| p[k]).min().unwrap());
    let max = [0, 1, 2].map(|k| s.iter().map(|p| p[k]).max().unwrap());
    assert_eq!((min, max), ([22, 23, 25], [34, 35, 38]));
    assert_eq!(lit(&c, [min[0], min[1], min[2]]), [42, 45, 49]);
    assert_eq!(lit(&c, [max[0], max[1], max[2]]), [66, 68, 74]);
    assert_eq!(c.level(29.0), 56);
    let (_, s) = face(&militia, 2366);
    assert!(s.iter().all(|p| *p == [8, 8, 8]));
    assert!(s.iter().all(|p| lit(&c, *p) == [15, 15, 15]));

    // cs_militia's sky colour from light_environment (mdl.md).
    let sky = sky_of(&militia).expect("a light_environment");
    assert_eq!(sky.color, [229.0, 229.0, 151.0]);
}

#[test]
fn the_converted_map_carries_the_curve() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_aztec", true);
    let atlas = map.lightmap.as_ref().expect("a lightmap");
    assert_eq!(atlas.curve, Some(LightCurve::default()));
    assert!(map.textures.iter().any(|t| t.name == "{vine1"), "the vines are in the map");
}

#[test]
fn a_model_on_the_hostage_rooms_floor_takes_the_floors_light() {
    let Some(mount) = mount() else { return };
    let world = goldsrc_engine::content::model_light::World::new(bsp(&mount, "cs_militia"), |s| {
        if s == 0 { 264.0 } else { 0.0 }
    });
    // mdl.md's worked case: stored about 29 at m, so C 29, T 29 (the
    // floor's samples run from (22, 23, 25) to (34, 35, 38)).
    let c = world.floor_light([454.0, 862.0, -160.0 + 8.0]).expect("a lit floor under the point");
    assert!(c.iter().all(|v| (20.0..=40.0).contains(v)), "{c:?}");
    let lit = world.light_at([454.0, 862.0, -160.0]);
    let t = lit.ambient + lit.shade;
    assert!((20.0..=40.0).contains(&t), "{t}");
    assert!((lit.shade - 0.9 * t).abs() < 0.01);
}
