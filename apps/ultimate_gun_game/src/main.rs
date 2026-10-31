//! The Ultimate Gun Game binary (`ultimate_gun_game`): Lucker Party's
//! client app (`party_kit::client_app`) with the Ultimate Gun Game's
//! parts (`UltimateGunGamePlugin::launch`: a fresh seed at every launch,
//! printed; a new ladder and a random map at every rotation). It starts
//! at the party's main menu (`party_kit::main_menu`: Play hosts the
//! automatic gun game, Join connects); `--map`, `+minigame`, `+connect`
//! and the tool runs (`--screenshot` ...) skip it and play the automatic
//! gun game on its pool's first map (`--map` picks another, `+minigame`
//! another minigame) as before. The options are the shell's (`--help`).

use chimera_client::client::Args;

fn main() {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        println!(
            "ultimate_gun_game: a gun game whose ladder is every composed game's guns; each gun makes you its\n\
             game. Starts {} on its first map; `ugg_new [seed] [rungs]` (console) a new ladder, `parts`\n\
             what each game lends. The options are the shell's (below).\n",
            ultimate_gun_game::MINIGAME,
        );
    }
    let args = Args::parse();
    // Without a map, minigame, server or capture on the command line: the
    // main menu (`party_kit::main_menu`); else straight into the game as
    // before.
    let at_menu = party_kit::main_menu::starts_at_menu(&args);
    let first = |id: &str| ultimate_gun_game::first_map(id);
    let args = if at_menu { args } else { party_kit::with_defaults_for(args, ultimate_gun_game::MINIGAME, &first) };
    let problems = party_kit::requirements();
    if !problems.is_empty() {
        let (text, fatal) = party_kit::report(&problems);
        if fatal {
            party_kit::exit_fatal(&text);
        }
        eprintln!("{text}");
    }
    let mut app = if at_menu {
        party_kit::client_app_at_menu(args, "Ultimate Gun Game", ultimate_gun_game::MINIGAME)
    } else {
        let mut app = party_kit::client_app(args, "Ultimate Gun Game");
        app.add_plugins(party_kit::main_menu::MainMenuPlugin {
            at_start: false,
            minigame: ultimate_gun_game::MINIGAME.into(),
            keep_recent: false,
        });
        app
    };
    app.add_plugins((
        ultimate_gun_game::UltimateGunGamePlugin::launch(),
        chimera_client::client::hud_minimal::MinimalHudPlugin,
    ));
    chimera_client::client::app::run(app);
}
