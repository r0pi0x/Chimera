//! The Ultimate Gun Game binary (`ultimate_gun_game`): the crate's `APP`
//! (lib.rs: Lucker Party's client app, `party_kit::client::PARTY`, with
//! the Ultimate Gun Game's parts, `UltimateGunGamePlugin::launch`: a
//! fresh seed at every launch, printed; a new ladder and a random map at
//! every rotation) on Chimera's client app (`chimera_client::client::
//! app::run`). It starts at the party's main menu (`party_kit::main_menu`:
//! Play hosts the automatic gun game, Join connects); `--map`,
//! `+minigame`, `+connect` and the tool runs (`--screenshot` ...) skip it
//! and play the automatic gun game on its pool's first map (`--map` picks
//! another, `+minigame` another minigame) as before. Options: `--help`.

fn main() {
    chimera_client::client::app::run(&ultimate_gun_game::APP);
}
