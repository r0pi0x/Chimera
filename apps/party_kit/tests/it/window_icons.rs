//! The party's window icons (`client::WINDOW_ICONS`) are real PNGs that
//! decode to square images, each opaque on its avatar ground.

use party_kit::client::WINDOW_ICONS;

#[test]
fn every_icon_decodes_square() {
    assert_eq!(WINDOW_ICONS.len(), 3);
    for png in WINDOW_ICONS {
        let img = image::load_from_memory(png).expect("icon decodes").into_rgba8();
        assert_eq!(img.width(), img.height());
        assert_eq!(img.width(), 256);
        assert!(chimera_client::client::window_icon::decode(png).is_some());
    }
}
