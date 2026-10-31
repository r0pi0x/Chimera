//! The render-style reader (specs/combat_arms/models_ltb.md "Render-style
//! file", Follow-up (2026-10-31) 1, case R6) on files written in code: the
//! shader-options byte follows **each** pass record, not the pass list.

use lithtech_formats::render_style::{Material, Pass, RenderStyleFile, Stage, Style, write};

fn pass(blend: u32, depth: u32, lit: bool) -> Pass {
    let mut p = Pass {
        blend,
        depth,
        cull: 1,
        factor: 0x8080_8080,
        dynamic_lighting: lit,
        ..Pass::default()
    };
    p.stages[0] = Stage {
        texture: 1,
        color_op: 4,
        color_arg1: 2,
        color_arg2: 1,
        ..Stage::default()
    };
    p
}

fn style(passes: Vec<Pass>) -> Style {
    Style {
        material: Material {
            ambient: [0.333, 0.333, 0.333, 1.0],
            diffuse: [0.875, 0.875, 0.875, 1.0],
            emissive: [0.416, 0.416, 0.416, 0.0],
            specular: [0.443, 0.443, 0.443, 0.0],
            power: 50.0,
        },
        passes,
        effect: Some((false, 0)),
    }
}

/// Three passes, a zero shader-options byte after each: 20 + 8 + 4 + 68 +
/// 4 + 3 x 533 + 8 = 1,711 bytes, as `RS\PV_CHAR_B.LTB`.
#[test]
fn three_passes_each_followed_by_its_options_byte() {
    let file = RenderStyleFile {
        styles: vec![style(vec![pass(0, 0, true), pass(1, 1, false), pass(10, 1, false)])],
    };
    let bytes = write(&file);
    assert_eq!(bytes.len(), 1_711);
    assert_eq!(RenderStyleFile::parse(&bytes).unwrap(), file);
}

/// A pass whose options byte is non-zero has 10 more bytes after it (the
/// stock `default` style asks for pixel shader 0): the next pass starts
/// after them, not right after the byte.
#[test]
fn a_nonzero_options_byte_brings_ten_bytes_in_its_pass() {
    let mut first = pass(0, 0, true);
    first.shaders = Some((false, 0, true, 0));
    let file = RenderStyleFile {
        styles: vec![style(vec![first, pass(1, 1, false)])],
    };
    let bytes = write(&file);
    assert_eq!(bytes.len(), 20 + 8 + 4 + 68 + 4 + (533 + 10) + 533 + 8);
    let back = RenderStyleFile::parse(&bytes).unwrap();
    assert_eq!(back, file);
    assert_eq!(back.styles[0].passes[0].shaders, Some((false, 0, true, 0)));
    assert_eq!(back.styles[0].passes[1].shaders, None);
    // One pass with the options: 655 bytes, as `RS\default.ltb`.
    let mut only = pass(0, 0, true);
    only.shaders = Some((false, 0, true, 0));
    let one = RenderStyleFile { styles: vec![style(vec![only])] };
    assert_eq!(write(&one).len(), 655);
}

#[test]
fn refuses_what_is_not_a_render_style() {
    let good = write(&RenderStyleFile { styles: vec![style(vec![pass(0, 0, true)])] });
    assert!(RenderStyleFile::parse(&good).is_ok());
    let mut model = good.clone();
    model[0] = 1;
    assert!(RenderStyleFile::parse(&model).is_err(), "a model");
    let mut old = good.clone();
    old[2] = 2;
    assert!(RenderStyleFile::parse(&old).is_err(), "version 2");
    assert!(RenderStyleFile::parse(&good[..good.len() - 1]).is_err(), "cut short");
    let mut more = good.clone();
    more.push(0);
    assert!(RenderStyleFile::parse(&more).is_err(), "bytes left over");
    let mut five = good.clone();
    five[20 + 8 + 4 + 68..20 + 8 + 4 + 68 + 4].copy_from_slice(&5u32.to_le_bytes());
    assert!(RenderStyleFile::parse(&five).is_err(), "5 passes");
}

/// 13 stock files end after their passes (no effect options): they read,
/// with none.
#[test]
fn effect_options_are_optional() {
    let mut file = RenderStyleFile { styles: vec![style(vec![pass(0, 0, true)])] };
    file.styles[0].effect = None;
    let bytes = write(&file);
    assert_eq!(bytes.len(), 20 + 8 + 4 + 68 + 4 + 533);
    assert_eq!(RenderStyleFile::parse(&bytes).unwrap(), file);
}
