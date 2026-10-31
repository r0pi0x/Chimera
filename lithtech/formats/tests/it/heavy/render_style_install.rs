//! The render-style reader on the user's Combat Arms install (models_ltb.md
//! Follow-up (2026-10-31) case R6): every `RS` file reads to its last byte
//! (the shader-options byte after each pass), and `PV_CHAR_B.LTB` is its
//! three passes. Passes with a note without an install.

use lithtech_formats::{
    render_style::RenderStyleFile,
    rez::{RezArchive, RezKeys},
};
use chimera_vfs::FileSource;

use super::rez_install::game_dir;

fn archive() -> Option<RezArchive> {
    let rez = RezArchive::open(game_dir()?.join("RS.rez"), &RezKeys::new()).ok()?;
    if rez.header().key_scheme().is_some() {
        eprintln!("skipping: RS.rez is keyed");
        return None;
    }
    Some(rez)
}

/// R6: 1,711 bytes, 3 passes, a zero options byte after each; pass 1 lit,
/// blend 0, depth 0; pass 2 unlit, add (1), depth 1, stage 2 texture 2 with
/// colour op 14; pass 3 unlit, multiply (10), depth 1, stage 2 texture 2,
/// colour op 15, coordinates from the world-space normal, clamped.
#[test]
fn r6_pv_char_b() {
    let Some(rez) = archive() else { return };
    let bytes = rez.read("rs/pv_char_b.ltb").unwrap().expect("RS\\PV_CHAR_B.LTB");
    assert_eq!(bytes.len(), 1_711);
    let f = RenderStyleFile::parse(&bytes).unwrap();
    assert_eq!(f.styles.len(), 1);
    let s = &f.styles[0];
    assert_eq!(s.passes.len(), 3);
    assert!(s.passes.iter().all(|p| p.shaders.is_none()), "zero options bytes");
    let [p1, p2, p3] = [&s.passes[0], &s.passes[1], &s.passes[2]];
    assert_eq!((p1.dynamic_lighting, p1.blend, p1.depth), (true, 0, 0));
    assert_eq!((p2.dynamic_lighting, p2.blend, p2.depth), (false, 1, 1));
    assert_eq!((p2.stages[2].texture, p2.stages[2].color_op), (2, 14));
    assert_eq!((p3.dynamic_lighting, p3.blend, p3.depth), (false, 10, 1));
    assert_eq!((p3.stages[2].texture, p3.stages[2].color_op), (2, 15));
    assert_eq!((p3.stages[2].coord_source, p3.stages[2].u_address), (7, 1));
    assert_eq!(p1.factor, 0x8080_8080);
    assert!((s.material.power - 50.0).abs() < 1e-3 && (s.material.specular[0] - 0.443).abs() < 1e-3);
}

/// Every style file of the archive reads to its last byte; `default` is
/// the one with a shader-options byte set (655 bytes).
#[test]
fn every_render_style_file_reads() {
    let Some(rez) = archive() else { return };
    let mut n = 0;
    for e in rez.entries().into_iter().filter(|e| e.path.ends_with(".ltb")) {
        let bytes = rez.read(&e.path).unwrap().unwrap();
        // Two files in the folder are other things (byte 0 is 0).
        if bytes[0] != 5 {
            continue;
        }
        let f = RenderStyleFile::parse(&bytes).unwrap_or_else(|err| {
            panic!("{}: {err} (len {})", e.path, bytes.len())
        });
        n += 1;
        if e.path == "rs/default.ltb" {
            assert_eq!(bytes.len(), 655);
            assert_eq!(f.styles[0].passes[0].shaders.map(|s| s.2), Some(true), "pixel shader on");
        }
    }
    eprintln!("{n} render-style files read");
    assert!(n >= 60, "{n} files");
}
