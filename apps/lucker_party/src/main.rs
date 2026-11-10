//! The Lucker Party game binary (`lucker_party`): the party kit's client
//! app (`party_kit::client::APP`) on Chimera's client app
//! (`chimera_client::client::app::run`): a minigame (the gun game on
//! de_dust2 without `+minigame`) played as Combat Arms' soldiers with
//! both games' guns on its pool's first map (`--map` picks another).
//! Options: `--help`; `+minigame <id>` the minigame, `minigames`
//! (console) lists them.

fn main() {
    chimera_client::client::app::run(&party_kit::client::APP);
}
