//! DoD:S's front end's tests: what only the client shows (its
//! registration; the HUD's frame is `hud`'s unit test).

#[test]
fn the_client_is_dods_with_its_hud() {
    assert_eq!(dods_ui::party::CLIENT.game, dods::game::GAME);
    assert!(dods_ui::party::CLIENT.hud.is_some(), "DoD:S's HUD (M3b)");
}
