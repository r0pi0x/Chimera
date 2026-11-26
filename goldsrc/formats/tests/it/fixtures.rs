//! The specs' worked examples that need no game files: a hull trace on a
//! one-plane clip tree (bsp.md, "Test cases"), the visibility row
//! decoding, triangle commands, `liblist.gam`, `materials.txt` matching
//! the HUD sprite lists, and alpha-tested images' holes (a `{` mip
//! texture, a masked studio texture, an alpha-test sprite: wad3.md,
//! "Converting to RGBA"), each built in code.

use goldsrc_formats::bsp::hull::{CONTENTS_SOLID, HULL_BOXES};
use goldsrc_formats::bsp::solids::{Convex, HalfSpace, Shrink, shrink};
use goldsrc_formats::bsp::{Bsp, ClipNode, Model, Plane, parse_entities, vis::decode_row, wad_names};
use goldsrc_formats::liblist::LibList;
use goldsrc_formats::materials::{Materials, match_name};
use goldsrc_formats::mdl::{Texture, TriCommand, TriVertex};
use goldsrc_formats::miptex::MipTex;
use goldsrc_formats::spr::{Frame, FrameEntry, Sprite, TextureFormat, parse_sprite_list};

/// A hull 1 tree for a flat floor at z = 0: its hull 1 plane at z = 36,
/// empty above, solid below.
fn floor() -> Bsp {
    Bsp {
        planes: vec![Plane {
            normal: [0.0, 0.0, 1.0],
            dist: 36.0,
            kind: 2,
        }],
        clip_nodes: vec![ClipNode {
            plane: 0,
            children: [-1, CONTENTS_SOLID as i16],
        }],
        models: vec![Model {
            mins: [-64.0; 3],
            maxs: [64.0; 3],
            origin: [0.0; 3],
            heads: [0, 0, 0, 0],
            vis_leaves: 0,
            first_face: 0,
            face_count: 0,
        }],
        ..Bsp::default()
    }
}

#[test]
fn a_hull_trace_stops_a_32nd_short_of_the_floor() {
    let t = floor().trace(0, 1, [0.0, 0.0, 100.0], [0.0, 0.0, 0.0]);
    assert!((t.fraction - 0.6396875).abs() < 1e-9, "{}", t.fraction);
    assert!((t.end[2] - 36.03125).abs() < 1e-9);
    assert_eq!(t.plane_normal, [0.0, 0.0, 1.0]);
    assert!(!t.started_solid && !t.all_solid && t.in_open);
}

#[test]
fn a_hull_trace_starting_in_solid_is_all_solid() {
    let t = floor().trace(0, 1, [0.0, 0.0, 20.0], [0.0, 0.0, 0.0]);
    assert!(t.started_solid && t.all_solid);
}

#[test]
fn point_contents_and_an_untouched_trace() {
    let b = floor();
    assert_eq!(b.contents(0, 1, [0.0, 0.0, 36.5]), -1);
    assert_eq!(b.contents(0, 1, [0.0, 0.0, 35.5]), CONTENTS_SOLID);
    let t = b.trace(0, 1, [0.0, 0.0, 100.0], [10.0, 0.0, 50.0]);
    assert_eq!(t.fraction, 1.0);
    assert_eq!(t.end, [10.0, 0.0, 50.0]);
}

#[test]
fn the_floors_solid_is_one_region_and_shrinks_back_by_the_box() {
    let b = floor();
    let regions = goldsrc_formats::bsp::solids::convex_regions(&b, 0, 1);
    assert_eq!(regions.len(), 1);
    let (_, c) = &regions[0];
    assert!((c.maxs[2] - 36.0).abs() < 1e-6, "{:?}", c.maxs);
    let half = HULL_BOXES[1].1.map(|v| v as f64);
    let s = shrink(c, half, Shrink::Measured, |_, _| true).unwrap();
    assert!((s.maxs[2] - 0.0).abs() < 1e-6, "{:?}", s.maxs);
    // A standing box 0.5 above the floor's top is clear; 0.5 into it isn't.
    assert!(s.box_overlap([0.0, 0.0, 36.5], half) <= 0.0);
    assert!((s.box_overlap([0.0, 0.0, 35.5], half) - 0.5).abs() < 1e-6);
    let t = s.box_sweep([0.0, 0.0, 100.0], [0.0, 0.0, -100.0], half).unwrap();
    assert!((t - 0.64).abs() < 1e-9, "{t}");
}

#[test]
fn a_cut_cube_has_eight_corners_and_its_diagonal_cut_seven_faces() {
    let cube = Convex::cut(&[], [-8.0; 3], [8.0; 3]).unwrap();
    assert_eq!((cube.planes.len(), cube.vertices.len()), (6, 8));
    let n = 1.0 / 3f64.sqrt();
    let cut = Convex::cut(
        &[HalfSpace {
            normal: [n, n, n],
            dist: 8.0 * n * 2.0,
        }],
        [-8.0; 3],
        [8.0; 3],
    )
    .unwrap();
    assert_eq!(cut.planes.len(), 7);
    assert_eq!(cut.vertices.len(), 10);
}

#[test]
fn visibility_rows_decode_with_zero_runs() {
    // Bits for leaves 1, 2, 3, 6, 8; then 5 zero bytes; ...
    let row = [0b1010_0111, 0, 5, 0b0000_0001, 0b0001_0000, 0, 12, 0b1000_0000];
    assert_eq!(decode_row(&row, 256), vec![1, 2, 3, 6, 8, 49, 61, 168]);
    // Excess bits past the last leaf are ignored.
    assert_eq!(decode_row(&[0xff], 3), vec![1, 2, 3]);
}

#[test]
fn entity_text_reads_blocks_and_the_last_key_wins() {
    let e = parse_entities(
        "{\n\"classname\" \"worldspawn\"\n\"wad\" \"\\sierra\\half-life\\valve\\halflife.wad;\\x\\cstrike\\cs_dust.wad;\"\n\"MaxRange\" \"8192\"\n\"classname\" \"worldspawn\"\n}\n{\n\"origin\" \"1 2 3\"\n\"classname\" \"info_player_start\"\n}\n",
    );
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].classname(), "worldspawn");
    assert_eq!(e[0].get("maxrange"), Some("8192"));
    assert_eq!(wad_names(&e[0]), vec!["halflife.wad", "cs_dust.wad"]);
    assert_eq!(e[1].get("origin"), Some("1 2 3"));
}

#[test]
fn triangle_strips_alternate_and_fans_turn_about_their_first_corner() {
    let v = |i| TriVertex {
        vertex: i,
        normal: 0,
        s: 0,
        t: 0,
    };
    let ids = |c: TriCommand| c.triangles().iter().map(|t| t.map(|v| v.vertex)).collect::<Vec<_>>();
    assert_eq!(
        ids(TriCommand::Strip(vec![v(0), v(1), v(2), v(3)])),
        vec![[0, 1, 2], [2, 1, 3]]
    );
    assert_eq!(
        ids(TriCommand::Fan(vec![v(0), v(1), v(2), v(3)])),
        vec![[0, 1, 2], [0, 2, 3]]
    );
}

#[test]
fn liblist_without_a_fallback_falls_back_to_valve() {
    let l = LibList::parse(
        "// comment\ngame \"Counter-Strike\"\nurl_dl \"\"\ncldll \"1\" \nmpentity \"info_player_start\"\nedicts\t\"1800\"\n",
    );
    assert_eq!(l.game(), Some("Counter-Strike"));
    assert_eq!(l.get("CLDLL"), Some("1"));
    assert_eq!(l.get("edicts"), Some("1800"));
    assert_eq!(l.mp_entity(), Some("info_player_start"));
    assert_eq!(l.folder_chain("cstrike", None), vec!["cstrike", "valve"]);
    assert_eq!(
        l.folder_chain("cstrike", Some("german")),
        vec!["cstrike_german", "cstrike", "valve"]
    );
}

#[test]
fn materials_match_on_twelve_characters_without_prefixes() {
    assert_eq!(match_name("-0SandRock2"), "sandrock2");
    assert_eq!(match_name("{pi_treeb"), "pi_treeb");
    assert_eq!(match_name("pl_out_shell4a"), "pl_out_shell");
    let m = Materials::parse("// x\nW pl_out_shell3\nM pl_out_shell4a\nD -0cstrike_me\nM SANDROCK2\n");
    assert_eq!(m.surface("pl_out_shell4a"), 'W');
    assert_eq!(m.surface("-0SandRock2"), 'M');
    assert_eq!(
        m.surface("-0cstrike_me"),
        'C',
        "entries written with a prefix never match"
    );
    assert_eq!(m.surface("unlisted"), 'C');
}

#[test]
fn a_hud_sprite_list_reads_its_rectangles() {
    let l = parse_sprite_list("2\nammo 320 320hud2 0 0 16 16\nammo 640 640hud7 72 72 24 24\n").unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(l[1].sprite, "640hud7");
    assert_eq!(l[1].rect, [72, 72, 24, 24]);
    // Fewer entries than counted (the install's hud.txt): those there.
    assert_eq!(parse_sprite_list("3\nammo 640 640hud7 72 72 24 24\n").unwrap().len(), 1);
    assert!(parse_sprite_list("1\nammo 640 640hud7 72\n").is_err());
}

/// A leafy 16 x 16 image's indices: a one-texel-wide stem down column 7
/// (index 1, green) on the key colour (255, blue) everywhere else, and
/// its palette.
fn leaf() -> (Vec<u8>, Vec<[u8; 3]>) {
    let px = (0..256).map(|i| if i % 16 == 7 { 1 } else { 255 }).collect();
    let mut palette = vec![[0, 0, 0]; 256];
    palette[1] = [40, 90, 20];
    palette[255] = [0, 0, 255];
    (px, palette)
}

/// What an alpha-tested image must decode to: the stem opaque and green,
/// every hole clear and green too (the stem's colour, not the key's blue
/// that filtering and the mip levels would mix in: de_aztec's blue grass),
/// so no texel is blue-dominant.
fn assert_holes_filled(rgba: &[u8], what: &str) {
    assert_eq!(rgba.len(), 256 * 4, "{what}");
    for (i, p) in rgba.chunks(4).enumerate() {
        assert_eq!(p[3], if i % 16 == 7 { 255 } else { 0 }, "{what}: texel {i} alpha");
        assert_eq!(&p[..3], &[40, 90, 20], "{what}: texel {i} colour");
    }
}

#[test]
fn alpha_tested_holes_take_the_opaque_colour_not_the_key() {
    let (px, palette) = leaf();
    // A `{` mip texture (its four levels; only level 0 is checked).
    let levels = vec![px.clone(), vec![255; 64], vec![255; 16], vec![255; 4]];
    let mut tex = MipTex {
        name: "{vine1".into(),
        width: 16,
        height: 16,
        mip_offsets: [40, 296, 360, 376],
        levels,
        palette: palette.clone(),
        colour_count: 256,
    };
    assert_holes_filled(&tex.rgba(0).unwrap(), "{vine1");
    // Without `{`, index 255 is an opaque blue texel.
    tex.name = "vine1".into();
    let plain = tex.rgba(0).unwrap();
    assert_eq!(&plain[..4], &[0, 0, 255, 255]);
    // A masked studio texture (flag 64).
    let skin = Texture {
        name: "leaf.bmp".into(),
        flags: 64,
        width: 16,
        height: 16,
        pixel_offset: 0,
        pixels: px.clone(),
        palette: palette.clone(),
    };
    assert_holes_filled(&skin.rgba(), "masked studio texture");
    // An alpha-test sprite (ikgrass1.spr's format).
    let frame = Frame {
        origin: [-8, 8],
        width: 16,
        height: 16,
        pixels: px,
    };
    let spr = Sprite {
        orientation: 0,
        format: TextureFormat::AlphaTest,
        radius: 11.3,
        max_width: 16,
        max_height: 16,
        beam_length: 0.0,
        sync: 0,
        palette,
        frames: vec![FrameEntry::Single(frame.clone())],
        trailing: 0,
    };
    assert_holes_filled(&spr.rgba(&frame), "alpha-test sprite");
}
