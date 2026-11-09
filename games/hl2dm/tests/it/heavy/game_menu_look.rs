//! HL2:DM's menus read from the install through Source's GameUI reader:
//! the file's own entries in its order, the title, the backgrounds, the
//! options' and Create Server's pages. The app draws these
//! (`source_gameui`); nothing of them is in the repository. Skips without
//! an install.

use source_engine::hud::gameui;

use super::map_stock::install_mount;

#[test]
fn the_menu_files_are_read_from_the_install() {
    let Some(mount) = install_mount() else { return };
    let ui = gameui::load(&mount, "resource/hl2mp_english.txt").expect("GameUI's files");
    let commands: Vec<String> = ui.menu.iter().map(|e| e.command.to_lowercase()).collect();
    for command in ["resumegame", "disconnect", "openserverbrowser", "opencreatemultiplayergamedialog", "openoptionsdialog", "quit"] {
        assert!(commands.iter().any(|c| c == command), "{command} in {commands:?}");
    }
    assert!(ui.menu.iter().all(|e| !e.label.starts_with('#')), "the entries' words are found");
    assert!(ui.menu.iter().find(|e| e.command == "ResumeGame").is_some_and(|e| e.in_game_only));
    assert_eq!(ui.title.len(), 2, "gameinfo.txt's two title lines: {:?}", ui.title);
    assert!(ui.title_font.is_some(), "the title's font");
    assert!(ui.background.is_some() && ui.background_wide.is_some(), "its backgrounds");
    for page in ["keyboard", "mouse", "audio", "video", "multiplayer", "create_server"] {
        assert!(ui.options.contains_key(page), "{page}: {:?}", ui.options.keys().collect::<Vec<_>>());
    }
    assert!(ui.server_settings.iter().any(|s| s.cvar == "mp_fraglimit"), "settings.scr");
    assert!(!ui.actions.is_empty(), "kb_act.lst");
    assert!(ui.loading.is_some(), "the loading dialog");
    assert!(!ui.servers.is_empty(), "the server browser's layouts");
}
