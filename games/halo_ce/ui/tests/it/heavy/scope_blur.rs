//! The scopes' blur from the disc's weapon HUDs (specs/halo_ce/hud.md 9,
//! S13), with the weapons' own field of view and magnifications
//! (`game::weapons::guns`): what Halo's HUD asks of the engine
//! (`draw::scope_blur`) for the pistol and the rocket launcher at 2x and
//! the sniper rifle at 2x and 8x (the radius 10.81, 7.92, 7.92 and 11; the
//! full-screen mask), nothing unzoomed, nothing for an unscoped weapon or
//! a dead unit. Passes with a note without an install.

use blam_formats::cache::MapFile;
use halo_ce::game::weapons::{self, guns};
use halo_ce_ui::hud::{
    data::HudData,
    draw::{self, Held, Unit},
    rules::WeaponNow,
};

fn data() -> Option<HudData> {
    let mount = super::disc::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(HudData::load(&MapFile::load(&bytes).unwrap()).unwrap())
}

/// The blur asked for while holding `id` at zoom level `zoom` (-1: none).
fn asked(d: &HudData, id: &str, zoom: i16, alive: bool) -> Option<draw::ScopeBlur> {
    let gun = guns().into_iter().find(|(g, _)| g == id).unwrap().1;
    let now = WeaponNow { zoom, ready: true, ..Default::default() };
    let unit = Unit { alive, weapon: Some(Held { tag: gun.tag.clone(), now }), ..Default::default() };
    let m = if zoom >= 0 { gun.magnification(zoom) } else { 1.0 };
    draw::scope_blur(d, &unit, draw::scope_fov(gun.camera_field_of_view, m))
}

#[test]
fn s13_each_scope_asks_for_its_blur() {
    let Some(d) = data() else { return };
    for (id, zoom, radius, mask) in [
        (weapons::PISTOL, 0, 10.81, [256.0, 256.0]),
        (weapons::ROCKET_LAUNCHER, 0, 7.92, [256.0, 256.0]),
        (weapons::SNIPER_RIFLE, 0, 7.92, [512.0, 256.0]),
        (weapons::SNIPER_RIFLE, 1, 11.0, [512.0, 256.0]),
    ] {
        let b = asked(&d, id, zoom, true).unwrap_or_else(|| panic!("{id} at {zoom}: a blur"));
        assert!((b.radius - radius).abs() < 0.01, "{id} at {zoom}: r {}", b.radius);
        assert_eq!(b.mask.size, mask, "{id}: the full-screen mask");
        assert!(d.pages[&b.mask.page].path.contains("mask"), "{}", d.pages[&b.mask.page].path);
        assert_eq!(asked(&d, id, -1, true), None, "{id} unzoomed: no pass at all");
        assert_eq!(asked(&d, id, zoom, false), None, "{id}: not for a dead unit");
    }
    assert_eq!(asked(&d, weapons::ASSAULT_RIFLE, -1, true), None, "no screen effect");
}
