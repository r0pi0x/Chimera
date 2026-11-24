//! Counter-Strike 1.6's front end's tests: what only the client shows (none yet but
//! its registration).

#[test]
fn the_client_is_the_game_s() {
    assert_eq!(cs16_ui::party::CLIENT.game, cs16::game::GAME);
}
