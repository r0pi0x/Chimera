//! The window icon's PNG decodes to a square image (the party's three).

use chimera_client::client::window_icon::decode;

#[test]
fn bad_png_is_skipped() {
    assert!(decode(b"not a png").is_none());
}
