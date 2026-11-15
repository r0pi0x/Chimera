//! The map converter's pure pieces on synthetic inputs (`game::maps`): the
//! coordinate, winding and yaw conversion (specs/halo_ce/cache_file.md,
//! "The world"), the lightmap atlas packer, the material-type table, the
//! fog conversion (shaders.md, "Atmospheric fog") and the cut-out bake.

use bevy::math::{Quat, Vec3};
use blam_formats::{cache::RealRgb, collision::MaterialType, scenario::Fog};
use halo_ce::game::maps::{
    lightmap::{Atlas, Page, UNWRITTEN, light_value},
    materials::bake_cutout,
    sky, space, surfaces,
};

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-5
}

/// The spec's frame: forward +X, left +Y, up +Z become the engine's -Z,
/// -X and +Y.
#[test]
fn halo_axes_map_onto_the_engines() {
    assert!(close(space::direction([1.0, 0.0, 0.0]), Vec3::NEG_Z), "forward");
    assert!(close(space::direction([0.0, 1.0, 0.0]), Vec3::NEG_X), "left");
    assert!(close(space::direction([0.0, 0.0, 1.0]), Vec3::Y), "up");
    // Right-handed to right-handed: forward x left = up on both sides.
    let (f, l, u) = (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y);
    assert!(close(f.cross(l), u));
}

/// One world unit is 3.048 m, and `to_game` undoes `point`.
#[test]
fn points_are_scaled_to_metres_and_back() {
    let p = [10.0, -2.0, 1.5];
    let e = space::point(p);
    assert!(close(e, Vec3::new(2.0, 1.5, -10.0) * 3.048));
    let back = space::to_game(e);
    for k in 0..3 {
        assert!((back[k] - p[k]).abs() < 1e-5);
    }
    let (lo, hi) = space::bounds([0.0, 1.0], [0.0, 2.0], [0.0, 3.0]);
    assert!(close(lo, Vec3::new(-2.0, 0.0, -1.0) * 3.048));
    assert!(close(hi, Vec3::new(0.0, 3.0, 0.0) * 3.048));
}

/// A stored render triangle is clockwise seen from its front (the normal's
/// side; structure_bsp.md); after the swap it is counter-clockwise from
/// the front in the engine: (p1 - p0) x (p2 - p0) points along the
/// mapped normal. A collision polygon (already counter-clockwise about
/// its outward normal) keeps its order and its facing.
#[test]
fn windings_end_counter_clockwise_from_the_front() {
    // Floor triangle seen from above (+Z): clockwise from above is
    // (0,0) -> (0,1) -> (1,0) in x, y.
    let tri = [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
    let normal = space::direction([0.0, 0.0, 1.0]);
    let [a, b, c] = space::render_triangle([0, 1, 2]).map(|i| space::point(tri[i as usize]));
    assert!((b - a).cross(c - a).dot(normal) > 0.0, "front is counter-clockwise");
    // Stored order would face down.
    let [a, b, c] = [0, 1, 2].map(|i: usize| space::point(tri[i]));
    assert!((b - a).cross(c - a).dot(normal) < 0.0);
    // Collision: counter-clockwise about +Z in Halo stays so.
    let ccw = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let [a, b, c] = ccw.map(space::point);
    assert!((b - a).cross(c - a).dot(Vec3::Y) > 0.0);
}

/// A heading's engine yaw looks where the heading's forward maps:
/// `Quat::from_rotation_y(yaw) * -Z` (the `Intent` convention). Facing
/// +Y (pi/2, Halo's left) looks along the engine's -X.
#[test]
fn headings_become_intent_yaws() {
    for h in [0.0f32, 0.3, 1.5707964, -1.2, 3.0, -3.1] {
        let yaw = space::yaw(h);
        let looking = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
        assert!(close(looking, space::direction([h.cos(), h.sin(), 0.0])), "heading {h}");
    }
    let looking = Quat::from_rotation_y(space::yaw(std::f32::consts::FRAC_PI_2)) * Vec3::NEG_Z;
    assert!(close(looking, Vec3::NEG_X));
}

fn page(w: u32, h: u32, seed: u32) -> Page {
    Page {
        width: w,
        height: h,
        rgb: (0..w * h)
            .map(|i| {
                let v = ((i + seed * 7919) % 251) as f32 / 251.0 + 0.001;
                [v, v * 0.5, seed as f32]
            })
            .collect(),
    }
}

/// The atlas: no two pages (with their borders) overlap; each border
/// texel repeats the page's nearest edge texel; a page coordinate lands
/// on the same texel in the atlas; texels no page covers are `UNWRITTEN`.
#[test]
fn the_atlas_packs_pages_with_copied_borders() {
    let sizes = [
        (256, 128),
        (128, 256),
        (64, 32),
        (8, 8),
        (512, 512),
        (32, 16),
        (3, 5),
        (1, 1),
    ];
    let mut atlas = Atlas::default();
    let pages: Vec<Page> = sizes
        .iter()
        .enumerate()
        .map(|(i, &(w, h))| page(w, h, i as u32 + 1))
        .collect();
    for p in &pages {
        atlas.add(p.clone());
    }
    let packed = atlas.finish();
    let (aw, ah) = (packed.map.width, packed.map.height);
    assert_eq!(packed.map.rgb.len(), (aw * ah) as usize);
    let rects: Vec<(u32, u32, u32, u32)> = packed
        .places
        .iter()
        .zip(&packed.sizes)
        .map(|(&(x, y), &(w, h))| (x, y, x + w + 2, y + h + 2))
        .collect();
    for (i, a) in rects.iter().enumerate() {
        assert!(a.2 <= aw && a.3 <= ah, "page {i} inside the atlas");
        for b in &rects[i + 1..] {
            let apart = a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1;
            assert!(apart, "{a:?} and {b:?} overlap");
        }
    }
    let mut covered = vec![false; (aw * ah) as usize];
    for (i, p) in pages.iter().enumerate() {
        let (px, py) = packed.places[i];
        for ty in 0..p.height + 2 {
            for tx in 0..p.width + 2 {
                let sx = tx.saturating_sub(1).min(p.width - 1);
                let sy = ty.saturating_sub(1).min(p.height - 1);
                assert_eq!(packed.texel(px + tx, py + ty), p.rgb[(sy * p.width + sx) as usize]);
                covered[((py + ty) * aw + px + tx) as usize] = true;
            }
        }
        // Each texel centre of the page maps onto the same texel's centre.
        for (x, y) in [(0, 0), (p.width - 1, p.height - 1), (p.width / 2, p.height / 3)] {
            let uv = [(x as f32 + 0.5) / p.width as f32, (y as f32 + 0.5) / p.height as f32];
            let [u, v] = packed.uv(i, uv);
            let (ax, ay) = (
                (u * aw as f32 - 0.5).round() as u32,
                (v * ah as f32 - 0.5).round() as u32,
            );
            assert_eq!((ax, ay), (px + 1 + x, py + 1 + y));
            assert!((u * aw as f32 - (ax as f32 + 0.5)).abs() < 1e-3);
            assert_eq!(packed.texel(ax, ay), p.rgb[(y * p.width + x) as usize]);
        }
    }
    for (i, c) in covered.iter().enumerate() {
        if !c {
            assert_eq!(packed.map.rgb[i], UNWRITTEN);
        }
    }
}

/// A stored lightmap byte is its sRGB value decoded: 0 is no light, 255
/// the texture's own brightness, mid-grey 128 about 0.216; no decoded
/// byte is the unwritten marker.
#[test]
fn lightmap_bytes_are_srgb_decoded() {
    assert_eq!(light_value(0), 0.0);
    assert!((light_value(255) - 1.0).abs() < 1e-6);
    assert!((light_value(128) - 0.2158).abs() < 1e-3);
    for b in 0..=255u8 {
        assert_ne!(light_value(b), UNWRITTEN[0]);
        if b > 0 {
            assert!(light_value(b) > light_value(b - 1));
        }
    }
    let p = Page::from_rgba8(1, 1, &[255, 128, 0, 255]);
    assert_eq!(p.rgb[0][0], 1.0);
    assert_eq!(p.rgb[0][2], 0.0);
}

/// The material-type table (data/surfaces.toml) covers the 33 codes in
/// collision.md's order, and names only the engine's generic surfaces.
#[test]
fn every_material_type_has_a_generic_surface() {
    let generic = [
        "concrete", "metal", "wood", "dirt", "sand", "grass", "snow", "glass", "tile", "plastic", "cloth", "flesh",
        "water", "default",
    ];
    let table = surfaces::table();
    assert_eq!(table.len(), 33);
    for t in MaterialType::ALL {
        let row = &table[&t.code()];
        assert!(generic.contains(&row.surface.as_str()), "{row:?}");
        assert_eq!(surfaces::surface_name(t).as_deref(), Some(row.surface.as_str()));
    }
    assert_eq!(
        surfaces::surface_name(MaterialType::MetalThick).as_deref(),
        Some("metal")
    );
    assert_eq!(surfaces::surface_name(MaterialType::Stone).as_deref(), Some("concrete"));
    assert_eq!(surfaces::surface_name(MaterialType::Glass).as_deref(), Some("glass"));
    assert_eq!(table[&0].name, "dirt");
    assert_eq!(table[&32].name, "hunter shield");
    assert_eq!(surfaces::surface_of_code(-1), None);
    assert_eq!(surfaces::surface_of_code(33), None);
}

/// The engine's fog from a Halo fog gives the game's factor at every
/// depth: clamp((d - start) / (opaque - start)) x density (the engine's
/// linear fog: min(max density, (d - start) / (end - start))).
#[test]
fn halo_fog_is_the_engines_linear_fog() {
    let halo = Fog {
        color: RealRgb {
            r: 0.533,
            g: 0.643,
            b: 0.710,
        },
        max_density: 0.2,
        start_distance: 0.0,
        opaque_distance: 32.0,
    };
    let f = sky::fog(&halo).unwrap();
    assert!(f.linear);
    assert_eq!(f.color, [0.533, 0.643, 0.710]);
    for d in [0.0f32, 1.0, 5.0, 16.0, 31.9, 32.0, 50.0, 400.0] {
        let m = d * 3.048;
        let engine = ((m - f.start) / (f.end - f.start)).min(f.max_density).clamp(0.0, 1.0);
        assert!(
            (engine - halo.factor(d)).abs() < 1e-5,
            "depth {d}: {engine} vs {}",
            halo.factor(d)
        );
    }
    // Off when the opaque distance is 0; density 0 counts as 1.
    assert!(
        sky::fog(&Fog {
            opaque_distance: 0.0,
            ..halo
        })
        .is_none()
    );
    let full = sky::fog(&Fog {
        max_density: 0.0,
        start_distance: 3.0,
        opaque_distance: 100.0,
        ..halo
    })
    .unwrap();
    assert_eq!(full.max_density, 1.0);
    assert!((full.start - 3.0 * 3.048).abs() < 1e-4 && (full.end - 100.0 * 3.048).abs() < 1e-3);
}

/// The cut-out bake (shaders.md, "Alpha testing"): a base texel's alpha is
/// 255 where the bump texel under it has alpha above 127, else 0; a bump
/// scale of 2 repeats the bump map twice across the base.
#[test]
fn the_bump_maps_alpha_cuts_the_base() {
    // Bump 2 x 1: left texel alpha 200, right 100.
    let bump = [0, 0, 0, 200, 0, 0, 0, 100];
    let mut base = vec![9u8; 4 * 4];
    bake_cutout(&mut base, 4, 1, &bump, 2, 1, [1.0, 1.0]);
    let alphas: Vec<u8> = base.chunks(4).map(|t| t[3]).collect();
    assert_eq!(alphas, [255, 255, 0, 0]);
    assert!(base.chunks(4).all(|t| t[..3] == [9, 9, 9]), "colour kept");
    let mut base = vec![9u8; 4 * 4];
    bake_cutout(&mut base, 4, 1, &bump, 2, 1, [2.0, 1.0]);
    let alphas: Vec<u8> = base.chunks(4).map(|t| t[3]).collect();
    assert_eq!(alphas, [255, 0, 255, 0]);
}

/// A placement's engine rotation carries the model's engine-space points
/// where the spec's rule (`position + p.x F + p.y L + p.z U`, in Halo's
/// axes) puts them: checked on the spec's worked orientations, then as a
/// whole composition with a translation.
#[test]
fn placement_transform_composes_in_engine_space() {
    use blam_formats::scenario::Orientation;
    use halo_ce::game::maps::objects::placement_rotation;
    let deg = |d: f32| d.to_radians();
    // Yaw 90: the model's forward (+X, engine -Z) turns to Halo +Y (engine
    // -X), its left (+Y, engine -X) to Halo -X (engine +Z).
    let q = placement_rotation(&Orientation::from_euler(deg(90.0), 0.0, 0.0));
    assert!(close(q * Vec3::NEG_Z, Vec3::NEG_X), "forward");
    assert!(close(q * Vec3::NEG_X, Vec3::Z), "left");
    assert!(close(q * Vec3::Y, Vec3::Y), "up");
    // The spec's second worked row: yaw 90, pitch 30 leaves F, tips U to
    // (-0.5, 0, 0.866) in Halo's axes.
    let o = Orientation::from_euler(deg(90.0), deg(30.0), 0.0);
    let q = placement_rotation(&o);
    assert!(
        close(q * Vec3::NEG_Z, space::direction([0.0, 1.0, 0.0])),
        "F unchanged by pitch"
    );
    assert!(
        close(q * Vec3::Y, space::direction([-0.5, 0.0, 0.8660254])),
        "up tipped back"
    );
    // A point through the whole chain equals the spec's rule then the
    // coordinate conversion.
    let o = Orientation::from_euler(deg(45.0), deg(30.0), deg(60.0));
    let q = placement_rotation(&o);
    let (position, p) = ([10.0, -20.0, 3.0], [1.0, 2.0, 3.0]);
    let r = o.rotate(p);
    let halo = [position[0] + r[0], position[1] + r[1], position[2] + r[2]];
    let engine = space::point(position) + q * space::point(p);
    assert!(
        close(engine, space::point(halo)),
        "{engine:?} vs {:?}",
        space::point(halo)
    );
    assert!((q.length() - 1.0).abs() < 1e-5, "a unit quaternion");
}

/// The sky model is baked at 1/1024 and the sky camera's scale undoes it:
/// a sky point times the scale is the model at its authored size, and the
/// models.md sky box (x -71793.6..55507.6, y -53461.3..86000.4, z up to
/// 98293.1 units) encloses a level's bounds after scaling (Blood Gulch's,
/// structure_bsp.md: x 5.9..131.9, y -190.2..-45.1, z -0.3..26.2).
#[test]
fn sky_scale_arithmetic() {
    assert_eq!(sky::SCALE, 1024.0);
    let p = [-71793.6, 86000.4, 98293.1];
    assert!(close(sky::sky_point(p) * sky::SCALE, space::point(p)));
    // The miniature spans about 70 x 84 x 96 units (213 m x ...).
    let (lo, hi) = (
        sky::sky_point([-71793.6, -53461.3, 0.0]),
        sky::sky_point([55507.6, 86000.4, 98293.1]),
    );
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let (level_lo, level_hi) = space::bounds([5.907, 131.902], [-190.219, -45.057], [-0.349, 26.233]);
    // The level's corners in the sky camera's scale are inside the model.
    assert!(lo.x * sky::SCALE < level_lo.x && hi.x * sky::SCALE > level_hi.x);
    assert!(lo.z * sky::SCALE < level_lo.z && hi.z * sky::SCALE > level_hi.z);
    assert!(hi.y * sky::SCALE > level_hi.y, "the dome is above the level");
    // One world unit of the level is 1/1024 of one of the miniature's.
    let a = sky::sky_point([1.0, 0.0, 0.0]).length();
    assert!((a * sky::SCALE - 3.048).abs() < 1e-4);
}
