//! The dedicated server starts on a stock map (`+map hl2dm:halls3`,
//! converted lean from the install) and serves it. Passes with a note
//! without the install.

use chimera_net::net;
use chimera_world::map::LoadedMapName;

#[test]
fn starts_on_a_stock_map() {
    let Some((mut server, _)) = crate::dedicated::server(&["+map", "hl2dm:halls3"]) else { return };
    server.app.update();
    assert_eq!(server.app.world().resource::<LoadedMapName>().0, "hl2dm:halls3");
    let status = net::status(server.app.world_mut());
    assert!(status.contains("halls3"), "{status}");
}
