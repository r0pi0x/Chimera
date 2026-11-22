//! The Ladder backdrop's guns drawn from the games' icon fonts as meshes
//! (`chimera_ui::icon_ink::glyph_shape`): HL2:DM's .357 (`.` of
//! `resource/hl2mp.ttf`) fills as the font rasterises it, the nonzero rule
//! (TrueType's). Its cylinder carries a square drawn as a thin gap around
//! a filled square, as FreeType draws it too: the small square seen on
//! rung 03 is the font's detail, not a fill error. Skips without an
//! install (`$CHIMERA_HL2DM_ROOT`, Steam or `[games.hl2dm] path`).

use bevy::prelude::*;
use chimera_ui::icon_ink::glyph_shape;
use hl2dm::game::{INFO, mount};
use hl2dm_ui::kill_icons::FONT;

/// The winding number of `contours` around `p` (a ray to +x).
fn winding(contours: &[Vec<Vec2>], p: Vec2) -> i32 {
    let mut w = 0;
    for c in contours {
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            let crosses = (a.y <= p.y) != (b.y <= p.y);
            if crosses && a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) > p.x {
                w += if b.y > a.y { 1 } else { -1 };
            }
        }
    }
    w
}

fn in_tri(t: &[Vec2; 3], p: Vec2) -> bool {
    let s = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
    let d = [s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0])];
    !(d.iter().any(|v| *v < 0.0) && d.iter().any(|v| *v > 0.0))
}

#[test]
fn the_357s_glyph_fills_as_the_font_does() {
    let Some(mount) = INFO.install().and_then(|i| mount::open(&i).ok()) else {
        eprintln!("skipping: no HL2:DM install (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)");
        return;
    };
    let font = mount.read(FONT).expect("the death notices' font");
    let g = glyph_shape(&font, '.').expect("the .357's glyph");
    // Every sample of a 200 x 100 grid over its ink: inside the triangles
    // exactly where the outline's nonzero winding is.
    let (nx, ny) = (200, 100);
    let mut wrong = 0;
    for iy in 0..ny {
        for ix in 0..nx {
            let p = g.ink.min + Vec2::new((ix as f32 + 0.5) / nx as f32, (iy as f32 + 0.37) / ny as f32) * g.ink.size();
            if (winding(&g.contours, p) != 0) != g.tris.iter().any(|t| in_tri(t, p)) {
                wrong += 1;
            }
        }
    }
    assert_eq!(wrong, 0, "samples where the mesh and the nonzero fill differ");
    // The cylinder's square (its one four-point contour): filled inside,
    // the gap around it empty, the body around that filled.
    let sq = g.contours.iter().find(|c| c.len() == 4).expect("the cylinder's square");
    let r = Rect::from_corners(sq[0], sq[2]);
    let filled = |p: Vec2| g.tris.iter().any(|t| in_tri(t, p));
    assert!(filled(r.center()), "the square's middle is ink");
    let gap = Vec2::new(r.min.x - 0.004, r.center().y);
    assert!(!filled(gap), "the thin gap around it is not");
    assert!(filled(Vec2::new(r.max.x + 0.04, r.center().y)), "the cylinder around that is");
}
