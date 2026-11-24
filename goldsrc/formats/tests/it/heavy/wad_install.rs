//! The WAD3 reader on the install, against specs/goldsrc/wad3.md: the
//! worked examples (`cstrike.wad`, `+0CSTRIKE_IE1LT`, the decal `{247`,
//! `CONCHARS`, `!toxicgrn`) and every mip texture of every WAD.

use goldsrc_formats::wad::{TYPE_FONT, TYPE_MIPTEX, Wad};

use super::install::{files, mount};

#[test]
fn cstrike_wad_and_its_first_texture() {
    let Some(m) = mount() else { return };
    let w = Wad::parse(m.read("cstrike.wad").unwrap()).unwrap();
    assert_eq!((w.entries.len(), w.dir_offset), (123, 2_677_428));
    let e = &w.entries[0];
    assert_eq!(
        (e.name.as_str(), e.kind, e.offset, e.disk_size),
        ("+0CSTRIKE_IE1LT", TYPE_MIPTEX, 12, 3_532)
    );
    let t = w.miptex("+0cstrike_ie1lt").unwrap().unwrap();
    assert_eq!((t.width, t.height, t.mip_offsets), (64, 32, [40, 2_088, 2_600, 2_728]));
    assert_eq!((t.palette[0], t.palette[255]), ([58, 55, 57], [51, 51, 51]));
    assert_eq!(t.levels[0].iter().filter(|&&i| i == 255).count(), 10);
    let rgba = t.rgba(0).unwrap();
    assert_eq!(
        rgba.chunks(4).filter(|p| p[3] == 255).count(),
        2_048,
        "index 255 stays opaque without `{{`"
    );
}

#[test]
fn decals_fonts_and_lookups_ignoring_case() {
    let Some(m) = mount() else { return };
    let root = super::install::root().unwrap();
    let decals = Wad::parse(std::fs::read(root.join("valve/decals.wad")).unwrap()).unwrap();
    let d = decals.miptex("{247").unwrap().unwrap();
    assert_eq!((d.width, d.height, d.palette[255]), (64, 64, [44, 0, 0]));
    assert_eq!(d.decal_rgba(0).unwrap()[3], 0);
    let gfx = Wad::parse(m.read("gfx.wad").unwrap()).unwrap();
    let e = gfx.find("conchars").unwrap();
    assert_eq!(e.kind, TYPE_FONT);
    let f = gfx.font(e).unwrap();
    assert_eq!((f.height, f.rows, f.row_height), (128, 6, 12));
    assert_eq!(
        (f.glyphs[b'A' as usize].offset, f.glyphs[b'A' as usize].width),
        (196, 7)
    );
    let hl = Wad::parse(m.read("halflife.wad").unwrap()).unwrap();
    let e = hl.find("!toxicgrn").unwrap();
    assert_eq!((e.name.as_str(), e.disk_size), ("!TOXICGRN", 6_252));
    assert_eq!(decals.entries.len(), 222);
    assert_eq!(
        Wad::parse(m.read("decals.wad").unwrap()).unwrap().entries.len(),
        225,
        "cstrike's shadows valve's"
    );
}

#[test]
fn every_mip_texture_of_every_wad_reads() {
    let wads: Vec<_> = files("cstrike", "wad")
        .into_iter()
        .chain(files("valve", "wad"))
        .collect();
    if wads.is_empty() {
        return;
    }
    let mut count = 0;
    for path in &wads {
        let w = Wad::parse(std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for e in w.entries.iter().filter(|e| e.kind == TYPE_MIPTEX) {
            let t = w
                .miptex(&e.name)
                .unwrap()
                .unwrap_or_else(|err| panic!("{} {}: {err}", path.display(), e.name));
            let a = t.width * t.height;
            assert_eq!(
                t.mip_offsets,
                [40, 40 + a, 40 + a + a / 4, 40 + a + a / 4 + a / 16],
                "{}",
                e.name
            );
            assert_eq!(t.colour_count, 256, "{}", e.name);
            count += 1;
        }
    }
    assert!(count >= 5_413, "{count} mip textures");
}
