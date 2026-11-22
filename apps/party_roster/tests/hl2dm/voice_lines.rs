//! Others' voice lines in 3D are on in the party apps (`party_voice_lines_3d
//! 1`, `chimera_world::map::sound::PartyVoices`; 0 in the standalone
//! games): the player's setting defaults to 1, has no Options row (the
//! user's call: it stays at the party's default), and reaches the server in each player's userinfo, so a party
//! server sends others' call-outs and radio calls to everyone near
//! (Combat Arms' and CS:S's own tests hear them: `combat_arms` grenades
//! `net_party_voice_lines_reach_others_who_asked`, `chimera_net`
//! radio_voices, `cs_source` ui radio_voices).

use chimera_core::console::Console;
use chimera_world::map::sound::{PARTY_VOICES, PartyVoices};

#[test]
fn the_party_hears_others_voice_lines_by_default() {
    let mut sim = crate::identity::party_net(20, 61);
    for c in &sim.clients {
        let w = c.app.world();
        assert_eq!(w.resource::<PartyVoices>().on, 1);
        let cvar = w.resource::<Console>().cvar(PARTY_VOICES).unwrap();
        assert_eq!((cvar.default.as_str(), cvar.archive, cvar.userinfo), ("1", true, true));
    }
    let players: Vec<_> = (0..2).map(|i| sim.character_of(i).unwrap()).collect();
    let server = sim.server.app.world();
    assert!(players.iter().all(|p| PartyVoices::wants(server, *p)), "the server knows each player asked");
    let options = include_str!("../../../party_kit/data/options.toml");
    assert!(!options.contains("cvar = \"party_voice_lines_3d\""), "no Options row: the party's default holds");
}
