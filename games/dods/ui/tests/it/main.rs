//! DoD:S's front end's tests: what only the client shows (none yet but
//! its registration).

#[test]
fn the_client_is_dods() {
    assert_eq!(dods_ui::party::CLIENT.game, dods::game::GAME);
    assert!(dods_ui::party::CLIENT.hud.is_none(), "no HUD of its own until M3");
}
