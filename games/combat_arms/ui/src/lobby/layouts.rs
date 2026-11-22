//! Our lobby layouts, in the game's layout format (`CBALAYOUT`), written
//! by code from the control names of specs/combat_arms/lobby.md (the
//! room list [`GAMELOBBY.XML`], the creation popup [`POPUP_CREATEROOM.XML`],
//! the waiting room [`GAMEROOM.XML`], the inventory [`INVENTORY.XML`], the
//! password box [`POPUP_LOCKROOM.XML`] and a message box). Places, sizes and
//! the flat stand-in textures are ours; the screen is drawn 1:1 on the
//! 1024 x 768 menu. Rows the game clones from a template (`column_RoomInfo`,
//! `UserInfoBar`) are a fixed number of numbered controls here.

use std::fmt::Write;

/// Room list rows shown.
pub const LIST_ROWS: usize = 18;
/// A team list's rows (T: 8 of 25 ref px).
pub const TEAM_ROWS: usize = 8;
/// Item rows in the inventory's list.
pub const ITEM_ROWS: usize = 12;
/// Chat lines shown in the lobby and the room.
pub const CHAT_LINES: usize = 7;

/// The room list cells' x offsets in a row (NO., TITLE, Map, Mode, SS,
/// Arms, Player(S), Status).
pub const LIST_COLS: [i32; 8] = [2, 40, 292, 414, 586, 618, 720, 802];
/// A team row's cells: the name, the status.
pub const TEAM_COLS: [i32; 2] = [6, 270];

/// A layout under construction.
struct X(String);

const TEXT_COLOUR: &str = "222,222,222";

impl X {
    fn new() -> Self {
        Self("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CBALAYOUT>\n".into())
    }

    fn window(&mut self, name: &str, pos: (i32, i32), size: (i32, i32), texture: &str) {
        let _ = writeln!(
            self.0,
            "<Control Type=\"Window\"><Property Name=\"{name}\" Position=\"{},{}\" Size=\"{},{}\" Texture=\"{texture}\"/>",
            pos.0, pos.1, size.0, size.1
        );
    }

    fn close(&mut self) {
        self.0.push_str("</Control>\n");
    }

    fn text(&mut self, name: &str, pos: (i32, i32), font: &str, colour: &str, wrap: i32) {
        let wrap = if wrap > 0 { format!(" WrapWidth=\"{wrap}\"") } else { String::new() };
        let _ = writeln!(
            self.0,
            "<Control Type=\"Text\"><Property Name=\"{name}\" Position=\"{},{}\" Font=\"{font}\" Element=\"\" \
             Normalcolor=\"{colour}\" MouseEvent=\"FALSE\"{wrap}/></Control>",
            pos.0, pos.1
        );
    }

    fn label(&mut self, name: &str, pos: (i32, i32), font: &str, text: &str) {
        let _ = writeln!(
            self.0,
            "<Control Type=\"Text\"><Property Name=\"{name}\" Position=\"{},{}\" Font=\"{font}\" Element=\"{text}\" \
             Normalcolor=\"{TEXT_COLOUR}\" MouseEvent=\"FALSE\"/></Control>",
            pos.0, pos.1
        );
    }

    /// A button with a centred label (the label is set by code when `label`
    /// is empty).
    fn button(&mut self, name: &str, pos: (i32, i32), size: (i32, i32), tex: &str, label: &str) {
        let _ = writeln!(
            self.0,
            "<Control Type=\"Button\"><Property Name=\"{name}\" Position=\"{},{}\" Size=\"{},{}\" ButtonTexture=\"{tex}\"/>\
             <Control Type=\"ButtonTypeText\"><Property Name=\"{name}_text\" Font=\"bold 12\" Element=\"{label}\" \
             HorizontalAlign=\"HCENTER\" VerticalAlign=\"VCENTER\" Normalcolor=\"210,210,210\" Overcolor=\"255,255,255\" \
             Clickcolor=\"255,198,0\" Disablecolor=\"110,110,110\"/></Control></Control>",
            pos.0, pos.1, size.0, size.1
        );
    }

    /// A left-aligned row button: the label starts at the left edge.
    fn row(&mut self, name: &str, pos: (i32, i32), size: (i32, i32)) {
        let _ = writeln!(
            self.0,
            "<Control Type=\"Button\"><Property Name=\"{name}\" Position=\"{},{}\" Size=\"{},{}\" ButtonTexture=\"opt_row_\"/>\
             <Control Type=\"ButtonTypeText\"><Property Name=\"{name}_text\" Font=\"bold 11\" Element=\"\" \
             HorizontalAlign=\"LEFT\" VerticalAlign=\"VCENTER\" Normalcolor=\"222,222,222\" Overcolor=\"255,255,255\" \
             Clickcolor=\"255,198,0\" Disablecolor=\"120,120,120\"/></Control></Control>",
            pos.0, pos.1, size.0, size.1
        );
    }

    /// A row button whose cells are texts at the given x offsets (the
    /// columns of a list, set by code).
    fn cells(&mut self, name: &str, pos: (i32, i32), size: (i32, i32), xs: &[i32]) {
        let _ = write!(
            self.0,
            "<Control Type=\"Button\"><Property Name=\"{name}\" Position=\"{},{}\" Size=\"{},{}\" ButtonTexture=\"opt_row_\"/>",
            pos.0, pos.1, size.0, size.1
        );
        for (j, x) in xs.iter().enumerate() {
            let _ = write!(
                self.0,
                "<Control Type=\"Text\"><Property Name=\"{name}_c{j}\" Position=\"{x},3\" Font=\"bold 11\" Element=\"\" \
                 Normalcolor=\"{TEXT_COLOUR}\" MouseEvent=\"FALSE\"/></Control>"
            );
        }
        self.0.push_str("</Control>\n");
    }

    fn check(&mut self, name: &str, pos: (i32, i32)) {
        let _ = writeln!(
            self.0,
            "<Control Type=\"CheckButton\"><Property Name=\"{name}\" Position=\"{},{}\" Size=\"14,14\" \
             ButtonTexture=\"opt_chk_\" CheckButtonType=\"RADIO\"/></Control>",
            pos.0, pos.1
        );
    }

    fn finish(mut self) -> String {
        self.0.push_str("</CBALAYOUT>\n");
        self.0
    }
}

/// The navigation bar every page has (N).
fn nav(x: &mut X) {
    x.button("button_lobby", (10, 40), (100, 24), "opt_tab_", "Lobby");
    x.button("button_gameroom", (114, 40), (100, 24), "opt_tab_", "Gameroom");
    x.button("button_character", (218, 40), (110, 24), "opt_tab_", "My Character");
    x.button("StartButton", (332, 40), (80, 24), "opt_tab_", "menu");
}

/// The chat area and its input line.
fn chat(x: &mut X, y: i32) {
    for i in 0..CHAT_LINES {
        x.text(&format!("chat_line_{i}"), (20, y + 16 * i as i32), "bold 11", TEXT_COLOUR, 0);
    }
    x.button("chat_input", (20, y + 16 * CHAT_LINES as i32 + 2), (500, 20), "opt_drop_", "");
}

/// The room list ("Game Lobby", GAMELOBBY.XML, L).
pub fn lobby() -> String {
    let mut x = X::new();
    x.window("gameLobby", (0, 0), (1024, 768), "lob_back");
    x.label("title", (20, 12), "bold 20", "Game Lobby");
    nav(&mut x);
    let cols: [(&str, &str, i32, i32); 8] = [
        ("Sort_Num", "NO.", 20, 36),
        ("Sort_Title", "TITLE", 58, 250),
        ("Sort_Map", "Map", 310, 120),
        ("Sort_Mode", "Mode", 432, 170),
        ("Sort_Mutation", "SS", 604, 30),
        ("Sort_Arms", "Arms", 636, 100),
        ("Sort_Players", "Player(S)", 738, 80),
        ("Sort_Status", "Status", 820, 90),
    ];
    for (name, label, px, w) in cols {
        x.button(name, (px, 76), (w, 22), "opt_btn_", label);
    }
    for i in 0..LIST_ROWS {
        let y = 100 + 22 * i as i32;
        x.cells(&format!("row_{i}"), (20, y), (892, 20), &LIST_COLS);
    }
    x.check("Check_join", (20, 504));
    x.label("Check_join_label", (40, 504), "bold 11", "Joinable");
    x.check("Check_pw", (140, 504));
    x.label("Check_pw_label", (160, 504), "bold 11", "No Password");
    x.check("Check_official", (290, 504));
    x.label("Check_official_label", (310, 504), "bold 11", "Pure Combat");
    x.button("mode_combo", (440, 500), (200, 20), "opt_drop_", "");
    x.button("map_combo", (650, 500), (200, 20), "opt_drop_", "");
    x.button("button_create", (20, 530), (130, 26), "opt_btn_", "Create Room");
    x.button("button_Join", (160, 530), (100, 26), "opt_btn_", "JOIN");
    x.text("list_count", (280, 536), "bold 11", TEXT_COLOUR, 0);
    chat(&mut x, 580);
    x.close();
    x.finish()
}

/// The waiting room (GAMEROOM.XML, R, O, T).
pub fn room() -> String {
    let mut x = X::new();
    x.window("gameroom", (0, 0), (1024, 768), "lob_back");
    x.text("room_title", (20, 12), "bold 20", "255,198,0", 0);
    nav(&mut x);
    x.button("Btn_Alpha", (20, 76), (360, 22), "opt_btn_", "Alpha");
    x.button("Btn_Bravo", (400, 76), (360, 22), "opt_btn_", "Bravo");
    x.text("mode_name", (440, 16), "bold 11", TEXT_COLOUR, 0);
    for i in 0..TEAM_ROWS {
        let y = 100 + 25 * i as i32;
        x.cells(&format!("alpha_{i}"), (20, y), (360, 22), &TEAM_COLS);
        x.cells(&format!("bravo_{i}"), (400, y), (360, 22), &TEAM_COLS);
    }
    x.button("button_teamChange", (20, 310), (130, 26), "opt_btn_", "Team Change");
    x.button("button_game_ready", (160, 310), (150, 26), "opt_btn_", "READY [F5]");
    x.button("button_game_start", (160, 310), (150, 26), "opt_btn_", "START [F5]");
    x.button("button_lobby_room", (320, 310), (110, 26), "opt_btn_", "To Lobby");
    x.button("button_invite", (440, 310), (90, 26), "opt_btn_", "Invite");
    // The Game Setting window.
    x.window("game setting", (780, 76), (236, 400), "lob_panel");
    x.label("set_title", (8, 4), "bold 12", "Game Setting");
    let rows = [
        ("drop_mode", "Mode", 28),
        ("drop_map", "Map", 72),
        ("drop_goal", "Goal", 116),
        ("drop_players", "Players", 160),
        ("drop_arms", "1st Arms", 204),
    ];
    for (name, label, y) in rows {
        x.label(&format!("{name}_label"), (8, y), "bold 11", label);
        x.button(name, (8, y + 14), (220, 20), "opt_drop_", "");
    }
    x.label("drop_second_arms_label", (8, 248), "bold 11", "2nd Arms");
    x.button("drop_second_arms", (8, 262), (220, 20), "opt_drop_", "-");
    let options = [
        ("Join", "Join Mid Combat", 292),
        ("TeamChange", "Swap Teams", 310),
        ("FriendlyFire", "Friendly Fire", 328),
        ("KillCam", "Kill Cam", 346),
        ("Perk", "Abilities", 364),
        ("Mutation", "Super Soldiers", 382),
    ];
    for (name, label, y) in options {
        x.label(&format!("{name}_label"), (8, y), "bold 11", label);
        x.check(&format!("{name}_On"), (130, y));
        x.label(&format!("{name}_On_label"), (148, y), "bold 11", "On");
        x.check(&format!("{name}_Off"), (180, y));
        x.label(&format!("{name}_Off_label"), (198, y), "bold 11", "Off");
    }
    x.close();
    x.text("room_info", (20, 350), "bold 11", TEXT_COLOUR, 0);
    chat(&mut x, 400);
    x.close();
    x.finish()
}

/// The room creation popup (POPUP_CREATEROOM.XML, C).
pub fn create() -> String {
    let mut x = X::new();
    x.window("CreateWnd", (262, 160), (500, 440), "opt_back");
    x.label("title", (20, 10), "bold 16", "Create Room");
    x.button("tab_multi", (20, 40), (150, 24), "opt_tab_", "MULTIPLAYER");
    x.button("tab_single", (174, 40), (200, 24), "opt_tab_", "PLAY WITH BOTS (Single)");
    x.label("name_label", (20, 84), "bold 12", "Name");
    x.button("Edit_RoomName", (150, 80), (320, 22), "opt_drop_", "");
    x.check("UsePW", (20, 120));
    x.label("pw_label", (40, 120), "bold 12", "Password");
    x.button("Edit_RoomPW", (150, 116), (320, 22), "opt_drop_", "");
    x.label("mode_label", (20, 160), "bold 12", "Mode");
    x.button("drop_mode", (150, 156), (320, 22), "opt_drop_", "");
    x.label("map_label", (20, 196), "bold 12", "Map");
    x.button("drop_map", (150, 192), (320, 22), "opt_drop_", "");
    x.text("goal_label", (20, 232), "bold 12", TEXT_COLOUR, 0);
    x.button("drop_goal", (150, 228), (320, 22), "opt_drop_", "");
    x.label("players_label", (20, 268), "bold 12", "PLAYER(S)");
    x.button("Combo_Players", (150, 264), (320, 22), "opt_drop_", "");
    x.text("arms_label", (20, 304), "bold 12", TEXT_COLOUR, 0);
    x.button("drop_arms", (150, 300), (320, 22), "opt_drop_", "");
    x.label("second_label", (20, 340), "bold 12", "2nd Arms");
    x.button("drop_second_arms", (150, 336), (320, 22), "opt_drop_", "-");
    x.button("CreateRoom_ok", (290, 390), (90, 26), "opt_btn_", "OK");
    x.button("button_cancel", (390, 390), (90, 26), "opt_btn_", "Cancel");
    x.close();
    x.finish()
}

/// The password box for a locked room (POPUP_LOCKROOM.XML).
pub fn password() -> String {
    let mut x = X::new();
    x.window("LockGameRoom", (362, 300), (300, 150), "opt_back");
    x.label("title", (20, 10), "bold 14", "Password");
    x.button("input_password", (20, 50), (260, 22), "opt_drop_", "");
    x.button("button_ok", (100, 100), (80, 26), "opt_btn_", "OK");
    x.button("button_cancel", (190, 100), (80, 26), "opt_btn_", "Cancel");
    x.close();
    x.finish()
}

/// A message box (a refusal, a failure).
pub fn message() -> String {
    let mut x = X::new();
    x.window("MessageBox", (312, 300), (400, 150), "opt_back");
    x.text("message_text", (20, 24), "bold 12", TEXT_COLOUR, 360);
    x.button("button_msg_ok", (160, 104), (80, 26), "opt_btn_", "OK");
    x.close();
    x.finish()
}

/// The start menu popup (`StartButton`, POPUP_STARTMENU.XML): the option
/// entry opens the options screen.
pub fn start_menu() -> String {
    let mut x = X::new();
    x.window("StartMenu", (332, 66), (200, 90), "opt_back");
    x.button("button_start_option", (20, 14), (160, 26), "opt_btn_", "Option");
    x.button("button_start_close", (20, 50), (160, 26), "opt_btn_", "Close");
    x.close();
    x.finish()
}

/// The inventory ("My Character", INVENTORY.XML, E).
pub fn inventory() -> String {
    let mut x = X::new();
    x.window("inventory", (0, 0), (1024, 768), "lob_back");
    x.label("title", (20, 12), "bold 20", "My Character");
    nav(&mut x);
    x.button("tab_Normal", (20, 76), (120, 24), "opt_tab_", "Normal");
    x.button("tab_Official", (144, 76), (120, 24), "opt_tab_", "Pure Combat");
    // The weapon slots (INVENTORY_CHA_WPN.XML).
    let slots = [
        ("equip_primary", "Primary", 110),
        ("equip_secondary", "Secondary", 136),
        ("equip_Melee", "Melee", 162),
        ("equip_Support", "Throw", 188),
        ("equip_backpackA", "Backpack A", 214),
        ("equip_backpackB", "Backpack B", 240),
        ("equip_backpackC", "Backpack C", 266),
        ("equip_backpackD", "Backpack D", 292),
    ];
    for (name, label, y) in slots {
        x.label(&format!("{name}_label"), (20, y + 4), "bold 11", label);
        x.button(name, (110, y), (250, 22), "opt_drop_", "");
    }
    // The gear slots (INVENTORY_CHA_GEA.XML).
    let gear = [
        ("equip_character", "Character", 110),
        ("equip_head", "Head", 136),
        ("equip_face", "Goggles", 162),
        ("equip_uniform", "Uniform", 188),
        ("equip_vest", "Vest", 214),
        ("equip_gear_backpack", "Backpack", 240),
    ];
    for (name, label, y) in gear {
        x.label(&format!("{name}_label"), (390, y + 4), "bold 11", label);
        x.button(name, (470, y), (250, 22), "opt_drop_", "");
    }
    // The ability slots (INVENTORY_CHA_PERK.XML).
    for i in 0..4 {
        let y = 292 + 26 * i;
        x.label(&format!("perk_{i}_label"), (390, y + 4), "bold 11", &format!("Ability {}", i + 1));
        x.button(&format!("equip_perk{i}"), (470, y), (250, 22), "opt_drop_", "");
    }
    // The list of what fits the chosen slot (INVENTORY_INVENTORY.XML).
    x.text("list_title", (20, 330), "bold 12", "255,198,0", 0);
    for i in 0..ITEM_ROWS {
        x.row(&format!("item_{i}"), (20, 352 + 22 * i as i32), (340, 20));
    }
    x.button("list_prev", (20, 620), (60, 22), "opt_btn_", "<");
    x.button("list_next", (90, 620), (60, 22), "opt_btn_", ">");
    x.button("button_equip", (170, 620), (90, 24), "opt_btn_", "Equip");
    x.button("button_release", (270, 620), (90, 24), "opt_btn_", "Release");
    x.text("inv_note", (20, 660), "bold 11", "255,120,120", 340);
    x.button("button_inv_back", (800, 700), (120, 26), "opt_btn_", "Back");
    x.close();
    x.finish()
}
