//! The SPR v2 reader on the install, against specs/goldsrc/spr.md: the
//! worked examples, every sprite parsing to its end, and `weapon_ak47.txt`.

use goldsrc_formats::spr::{FrameEntry, Sprite, TextureFormat, parse_sprite_list};

use super::install::{files, mount};

#[test]
fn worked_examples() {
    let Some(m) = mount() else { return };
    let s = Sprite::parse(&m.read("sprites/muzzleflash1.spr").unwrap()).unwrap();
    assert_eq!(
        (s.orientation, s.format, s.max_width, s.max_height, s.frames.len()),
        (2, TextureFormat::Additive, 48, 48, 1)
    );
    assert!((s.radius - 33.941).abs() < 1e-3);
    let FrameEntry::Single(f) = &s.frames[0] else { panic!() };
    assert_eq!((f.origin, f.width), ([-24, 24], 48));
    let smoke = Sprite::parse(&m.read("sprites/rifle_smoke1.spr").unwrap()).unwrap();
    assert_eq!((smoke.frames.len(), smoke.trailing), (30, 0));
    let black = Sprite::parse(&m.read("sprites/black_smoke1.spr").unwrap()).unwrap();
    let FrameEntry::Single(f) = &black.frames[0] else {
        panic!()
    };
    assert!(
        black
            .rgba(f)
            .chunks(4)
            .zip(&f.pixels)
            .all(|(p, &i)| p[..3] == [255, 255, 255] && p[3] == i)
    );
    let ch = Sprite::parse(&m.read("sprites/ch_sniper.spr").unwrap()).unwrap();
    let FrameEntry::Single(f) = &ch.frames[0] else { panic!() };
    assert_eq!(ch.rgba(f).chunks(4).filter(|p| p[3] == 0).count(), 64_429);
    let list = parse_sprite_list(&String::from_utf8_lossy(&m.read("sprites/weapon_ak47.txt").unwrap())).unwrap();
    assert_eq!(list.len(), 10);
    let ammo = list.iter().find(|e| e.name == "ammo" && e.resolution == 640).unwrap();
    assert_eq!((ammo.sprite.as_str(), ammo.rect), ("640hud7", [72, 72, 24, 24]));
}

#[test]
fn every_sprite_parses_to_its_end() {
    let all: Vec<_> = files("cstrike/sprites", "spr")
        .into_iter()
        .chain(files("valve/sprites", "spr"))
        .collect();
    if all.is_empty() {
        return;
    }
    for path in &all {
        let s = Sprite::parse(&std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!((s.palette.len(), s.trailing), (256, 0), "{}", path.display());
        assert!(
            s.frames.iter().all(|f| matches!(f, FrameEntry::Single(_))),
            "{}",
            path.display()
        );
    }
    assert!(all.len() >= 454, "{} sprites", all.len());
}
