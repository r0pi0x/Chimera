//! The network gun game's whole ladder with the definition's own respawn
//! delay and spawn protection (2 s each): `net_gun_game`'s fast test climbs
//! it with both cut short.

use crate::{net_gun_game, net_play};

#[test]
fn a_ladder_climbed_by_a_client_with_the_default_respawn_shows_on_both_clients() {
    net_gun_game::climbed_by_a_client(net_play::joined(), [20, 30, 25]);
}
