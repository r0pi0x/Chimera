//! CS:S's guns' kill icons register from the install at start, with no
//! map and no HUD look (`game_hud::register_install_icons`), as HL2:DM's
//! do: the menu's Ladder backdrop and another game's first HUD draw CS:S's
//! own glyphs instead of the fallback icons. Skips without an install.

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
use cs_source::{game::INFO, ui::game_hud::register_install_icons};

#[test]
fn guns_icons_register_from_the_install_without_a_hud() {
    if INFO.install().is_none() {
        eprintln!("skipping: no CS:S install");
        return;
    }
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default())).init_asset::<Font>().init_resource::<WeaponIcons>();
    app.world_mut().run_system_once(register_install_icons).unwrap();
    let icons = app.world().resource::<WeaponIcons>();
    let fonts = app.world().resource::<Assets<Font>>();
    for id in ["cs_source:weapon_deagle", "cs_source:weapon_glock", "cs_source:weapon_m3", "cs_source:weapon_ak47", "cs_source:weapon_knife"] {
        match icons.icon(id) {
            Some(WeaponIcon::Glyph { font, em, .. }) => {
                assert!(fonts.get(&font).is_some(), "{id}: its font is loaded");
                assert!(em > 0.0, "{id}: sized by the scheme");
            }
            other => panic!("{id}: no glyph from the install: {other:?}"),
        }
    }
    assert!(icons.mark("cs_source", chimera_client::client::kill_feed::HEADSHOT).is_some(), "the headshot mark");
    assert!(icons.glyph_fonts().next().is_some(), "the fonts to lay out ahead");
}
