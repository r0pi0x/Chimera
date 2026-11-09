//! CS:S's VGUI menus from the install, as neutral panel layouts
//! (`map::hud::GameMenus`): the buy menu (`resource/ui/buymenu_ter.res` /
//! `_ct`, the category pages its buttons open, each item's description
//! panel `classes/<item>.res`), the team menu (`resource/ui/teammenu.res`)
//! with the map's description (`maps/<map>.txt`), the vote panels (hl2's
//! `resource/ui/votehud.res`, each panel's inside as its own layout),
//! labels localised from `resource/cstrike_english.txt` (and the GameUI's
//! `GameUI_vote_*` words), pictures from `materials/vgui/`; and
//! the client scheme's text fonts (`fonts`, onto `GameHud::text_fonts`).
//!
//! The `.res` reader itself is Source's (`source_engine::hud::vgui`).

use std::collections::HashMap;

use super::hud::Kv;
use super::material::MaterialLoader;
use crate::map::hud::{CommandMenuItem, GameHud, GameMenus, HudSprite, UiKind, layout_key};
/// The generic `.res` reader (lifted: `source_engine::hud::vgui`).
pub use source_engine::hud::vgui::{fonts, hotkey, layout, merge, read_res};

/// Files of the buy menu's first page per team (ours: 1 terrorists, 2
/// CTs), and the page every team falls back to.
const BUY_PAGES: [(u8, &str); 2] = [(1, "resource/ui/buymenu_ter.res"), (2, "resource/ui/buymenu_ct.res")];
const BUY_FALLBACK: &str = "resource/ui/mainbuymenu.res";
const TEAM_MENU: &str = "resource/ui/teammenu.res";
const SCOREBOARD: &str = "resource/ui/scoreboard.res";
const SPECTATOR: &str = "resource/ui/spectator.res";
const SPECTATOR_MENU: &str = "resource/ui/bottomspectator.res";
const WIN_PANEL: &str = "resource/ui/win_round.res";
/// The vote panels (the engine's, hl2's; CS:S has no copy of its own) and
/// the ones drawn: the running vote, its passing, its failing, a call
/// refused.
const VOTE_HUD: &str = "resource/ui/votehud.res";
const VOTE_PANELS: [&str; 4] = ["VoteActive", "VotePassed", "VoteFailed", "CallVoteFailed"];
/// The message of the day's panel, and the class menus by team.
const MOTD_PANEL: &str = "resource/ui/motd.res";
const CLASS_PAGES: [(u8, &str); 2] = [(1, "resource/ui/classmenu_ter.res"), (2, "resource/ui/classmenu_ct.res")];
/// The install's message of the day as a server finds it (the game folder's
/// `motd.txt`, else the shipped default), and the plain-text one.
const MOTD_FILES: [&str; 3] = ["motd.txt", "cfg/motd.txt", "cfg/motd_default.txt"];
const MOTD_TEXT_FILES: [&str; 3] = ["motd_text.txt", "cfg/motd_text.txt", "cfg/motd_text_default.txt"];
/// The spectator menu's drop-downs' entries (command menu definitions).
const SPECTATOR_OPTIONS: &str = "resource/spectatormenu.res";
const SPECTATOR_MODES: &str = "resource/spectatormodes.res";

/// A command menu definition's entries: its `menuitem` blocks in order,
/// each with its label (`#token` resolved), command, toggle cvar and
/// submenu.
pub fn command_menu(root: &Kv, strings: &HashMap<String, String>) -> Vec<CommandMenuItem> {
    root.items()
        .iter()
        .filter(|(k, v)| k.to_lowercase().starts_with("menuitem") && matches!(v, Kv::Block(_)))
        .map(|(_, v)| {
            let label = v.str("label").unwrap_or("").trim();
            let label = match label.strip_prefix('#') {
                Some(t) => strings.get(&t.to_lowercase()).cloned().unwrap_or_else(|| t.to_string()),
                None => label.to_string(),
            };
            let text = |k: &str| v.str(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            CommandMenuItem {
                label,
                command: text("command"),
                toggle: text("toggle"),
                items: command_menu(v, strings),
            }
        })
        .collect()
}
/// The freeze cam's panel: its frame (`FreezePanelBG`), and what is
/// inside it laid out in the frame (`FREEZE_PANEL_INNER`).
const FREEZE_PANEL: &str = "resource/ui/freezepanel_basic.res";
const FREEZE_PANEL_INNER: &str = "resource/ui/freezepanel_basic.res#inner";

/// A path with its `..` segments resolved (`vgui/../vgui/x` -> `vgui/x`).
fn resolve_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." => {
                out.pop();
            }
            "." | "" => {}
            p => out.push(p),
        }
    }
    out.join("/")
}

/// An image panel's picture (`gfx/vgui/ak47`: under `materials/vgui/`) as
/// a sprite key, its texture loaded once.
fn image(materials: &mut MaterialLoader, sprites: &mut Vec<(String, HudSprite)>, path: &str) -> Option<String> {
    let name = resolve_dots(&format!("vgui/{}", path.trim().replace('\\', "/").to_lowercase()));
    if sprites.iter().any(|(k, _)| *k == name) {
        return Some(name);
    }
    // Resolving a missing material counts as a load warning: check first.
    materials.read(&format!("materials/{name}.vmt"))?;
    let texture = materials.resolve(&name).texture?;
    let t = &materials.textures[texture];
    let rect = [0.0, 0.0, t.width as f32, t.height as f32];
    sprites.push((name.clone(), HudSprite { texture, rect }));
    Some(name)
}

/// The menus from the install (None without its menu files). Pictures the
/// layouts show become `hud.sprites` (keyed by material name).
pub fn load(materials: &mut MaterialLoader, hud: &mut GameHud, map: &str) -> Option<GameMenus> {
    // The engine's strings (the spectator bars' `Spec_*`), then the game's
    // over them.
    let mut strings = HashMap::new();
    for file in ["resource/valve_english.txt", "resource/cstrike_english.txt"] {
        if let Some(b) = materials.read(file) {
            strings.extend(super::radio::localization(&super::radio::decode(&b)));
        }
    }
    // The GameUI's vote words (`GameUI_vote_*`: hl2's copy; cstrike's
    // `gameui_english.txt` lacks them), from every copy along the path.
    let path = crate::mount::normalize("resource/gameui_english.txt");
    for bytes in materials.mount().read_every(&path) {
        for (k, v) in super::radio::localization(&super::radio::decode(&bytes)) {
            if k.starts_with("gameui_vote") {
                strings.entry(k).or_insert(v);
            }
        }
    }
    let colors = hud.colors.clone();
    let mut menus = GameMenus::default();
    let mut sprites: Vec<(String, HudSprite)> = Vec::new();
    let read_res_at = |materials: &MaterialLoader, path: &str| {
        let mut read = |p: &str| materials.read(p).map(|b| super::radio::decode(&b));
        read_res(&mut read, path)
    };
    // Pages to read: the first pages, then what their buttons open.
    let mut queue: Vec<String> = BUY_PAGES.iter().map(|(_, p)| p.to_string()).collect();
    queue.push(BUY_FALLBACK.into());
    queue.push(TEAM_MENU.into());
    queue.push(MOTD_PANEL.into());
    queue.extend(CLASS_PAGES.iter().map(|(_, p)| p.to_string()));
    queue.extend([SCOREBOARD, SPECTATOR, SPECTATOR_MENU, WIN_PANEL].map(String::from));
    let mut seen = std::collections::HashSet::new();
    while let Some(path) = queue.pop() {
        let key = layout_key(&path);
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(root) = read_res_at(materials, &key) else {
            continue;
        };
        let mut l = layout(&root, &strings, &colors, &mut |p| image(materials, &mut sprites, p));
        for c in &mut l.controls {
            // The black market sticker shows only during a discount.
            if c.name.eq_ignore_ascii_case("MarketSticker") {
                c.visible = false;
            }
            if c.kind != UiKind::Button {
                continue;
            }
            if let Some(page) = c.command.as_ref().filter(|p| p.to_lowercase().ends_with(".res")) {
                queue.push(page.clone());
            }
            // A buy item's description panel, by the button's name.
            if c.keys
                .get("controlname")
                .is_some_and(|n| n.eq_ignore_ascii_case("MouseOverPanelButton"))
            {
                let info = format!("classes/{}.res", c.name.to_lowercase());
                if menus.layouts.contains_key(&info) {
                    c.info = Some(info);
                } else if let Some(root) = read_res_at(materials, &info) {
                    let info_layout = layout(&root, &strings, &colors, &mut |p| image(materials, &mut sprites, p));
                    menus.layouts.insert(info.clone(), info_layout);
                    c.info = Some(info);
                }
            }
        }
        menus.layouts.insert(key, l);
    }
    // The vote panels (hl2's `resource/ui/votehud.res`): their frames as
    // one layout, what is in each as its own (`VOTE_PANELS`).
    if let Some(root) = read_res_at(materials, VOTE_HUD) {
        let outer = layout(&root, &strings, &colors, &mut |_| None);
        for name in VOTE_PANELS {
            let block = root
                .items()
                .iter()
                .find(|(k, c)| k.eq_ignore_ascii_case(name) && matches!(c, Kv::Block(_)))
                .map(|(_, c)| c.clone());
            if let Some(block) = block {
                let inner = layout(&block, &strings, &colors, &mut |p| image(materials, &mut sprites, p));
                menus.layouts.insert(format!("{VOTE_HUD}#{name}"), inner);
            }
        }
        menus.layouts.insert(VOTE_HUD.into(), outer);
        menus.vote_hud = Some(VOTE_HUD.into());
    }
    for (key, s) in sprites {
        hud.sprites.insert(key, s);
    }
    for (team, path) in BUY_PAGES {
        if menus.layouts.contains_key(path) {
            menus.buy.insert(team, path.to_string());
        } else if menus.layouts.contains_key(BUY_FALLBACK) {
            menus.buy.insert(team, BUY_FALLBACK.to_string());
        }
    }
    if let Some(root) = read_res_at(materials, FREEZE_PANEL) {
        let outer = layout(&root, &strings, &colors, &mut |_| None);
        let frame = root
            .items()
            .iter()
            .find(|(k, c)| k.eq_ignore_ascii_case("FreezePanelBG") && matches!(c, Kv::Block(_)))
            .map(|(_, c)| c);
        if let Some(frame) = frame {
            let inner = layout(frame, &strings, &colors, &mut |_| None);
            menus.layouts.insert(FREEZE_PANEL.into(), outer);
            menus.layouts.insert(FREEZE_PANEL_INNER.into(), inner);
            menus.freeze_panel = Some((FREEZE_PANEL.into(), FREEZE_PANEL_INNER.into()));
        }
    }
    menus.team = menus.layouts.contains_key(TEAM_MENU).then(|| TEAM_MENU.to_string());
    menus.motd = menus.layouts.contains_key(MOTD_PANEL).then(|| MOTD_PANEL.to_string());
    for (team, path) in CLASS_PAGES {
        if menus.layouts.contains_key(path) {
            menus.class.insert(team, path.to_string());
        }
    }
    let first_file = |files: &[&str]| {
        files
            .iter()
            .find_map(|f| materials.read(f))
            .map(|b| super::radio::decode(&b))
            .filter(|t| !t.trim().is_empty())
    };
    menus.motd_file = first_file(&MOTD_FILES);
    menus.motd_text_file = first_file(&MOTD_TEXT_FILES);
    let have = |path: &str| menus.layouts.contains_key(path).then(|| path.to_string());
    (menus.scoreboard, menus.spectator, menus.spectator_menu) = (have(SCOREBOARD), have(SPECTATOR), have(SPECTATOR_MENU));
    menus.win_panel = have(WIN_PANEL);
    for (path, list) in [
        (SPECTATOR_OPTIONS, &mut menus.spectator_options),
        (SPECTATOR_MODES, &mut menus.spectator_modes),
    ] {
        if let Some(root) = read_res_at(materials, path) {
            *list = command_menu(&root, &strings);
        }
    }
    menus.map_info = materials
        .read(&format!("maps/{}.txt", map.to_lowercase()))
        .map(|b| super::radio::decode(&b).replace('\r', "").trim().to_string())
        .filter(|t| !t.is_empty());
    menus.strings = strings;
    (!menus.buy.is_empty() || menus.team.is_some()).then_some(menus)
}

#[cfg(test)]
mod tests {
    use super::super::hud::parse;
    use super::*;
    use crate::map::hud::HudCoord;

    const PAGE: &str = r##""Resource/UI/BuyPistols_TER.res"
{
    "Title" { "ControlName" "Label" "fieldName" "Title" "xpos" "52" "ypos" "22"
              "wide" "500" "tall" "48" "labelText" "#Cstrike_PistolsLabel" "font" "MenuTitle" }
    "Glock18" { "ControlName" "MouseOverPanelButton" "fieldName" "Glock18"
                "xpos" "52" "xpos" "0" "ypos" "116" "wide" "170" "tall" "20"
                "labelText" "#Cstrike_Glock18" "command" "buy glock" "cost" "400" }
    "Corner" { "ControlName" "Label" "xpos" "r40" "ypos" "c-10" "visible" "0"
               "labelText" "A && B" "fgcolor_override" "255 0 0 255" "bgcolor_override" "Orange" }
    "Pic" { "ControlName" "ImagePanel" "image" "gfx/vgui/glock18" "scaleImage" "1" }
    "Gadget" { "ControlName" "CSomethingElse" }
}"##;

    #[test]
    fn res_controls_with_strings_hotkeys_and_anchors() {
        let strings = HashMap::from([
            ("cstrike_pistolslabel".to_string(), "BUY PISTOLS".to_string()),
            ("cstrike_glock18".to_string(), "&1 9X19MM SIDEARM".to_string()),
        ]);
        let colors = HashMap::from([("Orange".to_string(), [255, 176, 0, 255])]);
        let mut images = Vec::new();
        let mut image = |p: &str| {
            images.push(p.to_string());
            Some(format!("vgui/{p}"))
        };
        let l = layout(&read_one(PAGE), &strings, &colors, &mut image);
        assert_eq!(l.controls.len(), 5);
        let title = l.get("title").unwrap();
        assert_eq!(
            (title.kind.clone(), title.text.as_str()),
            (UiKind::Label, "BUY PISTOLS")
        );
        assert_eq!(title.font.as_deref(), Some("MenuTitle"));
        let glock = l.get("Glock18").unwrap();
        assert_eq!(glock.kind, UiKind::Button);
        assert_eq!((glock.text.as_str(), glock.hotkey), ("1 9X19MM SIDEARM", Some('1')));
        assert_eq!(glock.command.as_deref(), Some("buy glock"));
        // A repeated key: the first one counts (as KeyValues finds it).
        assert_eq!(glock.x, HudCoord::Start(52.0));
        assert_eq!((glock.wide, glock.tall), (170.0, 20.0));
        assert_eq!(glock.keys.get("cost").map(String::as_str), Some("400"));
        let corner = &l.controls[2];
        assert_eq!(corner.name, "Corner", "no fieldName: the block's name");
        assert_eq!((corner.x, corner.y), (HudCoord::End(40.0), HudCoord::Centre(-10.0)));
        assert!(!corner.visible);
        assert_eq!((corner.text.as_str(), corner.hotkey), ("A & B", None));
        assert_eq!(corner.fg, Some([255, 0, 0, 255]));
        assert_eq!(corner.bg, Some([255, 176, 0, 255]));
        assert_eq!(l.controls[3].kind, UiKind::Image);
        assert_eq!(l.controls[3].image.as_deref(), Some("vgui/gfx/vgui/glock18"));
        assert_eq!(l.controls[4].kind, UiKind::Other("CSomethingElse".into()));
        assert_eq!(images, ["gfx/vgui/glock18"]);
    }

    fn read_one(text: &str) -> Kv {
        let mut read = |_: &str| Some(text.to_string());
        read_res(&mut read, "x.res").unwrap()
    }

    #[test]
    fn base_files_merge_under_the_files_own_keys() {
        let files = HashMap::from([
            (
                "classes/base_weapon.res",
                r##""classes/base_weapon.res" {
                    "price" { "ControlName" "Label" "xpos" "140" "labelText" "#Cstrike_GlockPrice" }
                    "classimage" { "ControlName" "ImagePanel" "image" "gfx/vgui/glock18" "wide" "256" }
                }"##,
            ),
            (
                "classes/ak47.res",
                r##"#base "base_weapon.res"
                "classes/ak47.res" {
                    "price" { "labelText" "#Cstrike_AK47Price" }
                    "classimage" { "image" "gfx/vgui/ak47" }
                    "extra" { "ControlName" "Label" }
                }"##,
            ),
        ]);
        let mut read = |p: &str| files.get(p).map(|s| s.to_string());
        let kv = read_res(&mut read, "classes/ak47.res").unwrap();
        let names: Vec<&str> = kv.items().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["price", "classimage", "extra"]);
        let price = kv.get("price").unwrap();
        assert_eq!(price.str("labelText"), Some("#Cstrike_AK47Price"));
        assert_eq!(price.str("xpos"), Some("140"), "kept from the base");
        assert_eq!(kv.get("classimage").unwrap().str("image"), Some("gfx/vgui/ak47"));
        assert_eq!(kv.get("classimage").unwrap().str("wide"), Some("256"));
        // A missing base is skipped.
        let mut none = |p: &str| (p == "a.res").then(|| "#base \"gone.res\" \"a\" { \"x\" \"1\" }".to_string());
        assert_eq!(read_res(&mut none, "a.res").unwrap().str("x"), Some("1"));
    }

    #[test]
    fn scheme_fonts_with_screen_heights() {
        let scheme = parse(
            r#"Scheme { Fonts {
                "Default" { "1" { "name" "Verdana" "tall" "12" "weight" "900" "yres" "480 599" }
                            "2" { "name" "Verdana" "tall" "9" "antialias" "1" } }
                "MenuTitle" { "1" { "name" "Verdana Bold" "tall" "18" "weight" "500" } }
            } }"#,
        );
        let f = fonts(&scheme.items()[0].1);
        assert_eq!(f["Default"].len(), 2);
        assert_eq!(f["Default"][0].yres, Some((480, 599)));
        assert_eq!((f["Default"][1].tall, f["Default"][1].weight), (9.0, 400));
        assert!(f["Default"][1].antialias && !f["Default"][0].antialias);
        assert_eq!(f["MenuTitle"][0].family, "Verdana Bold");
    }

    /// A command menu definition (the spectator menu's): items in order,
    /// labels localised, toggles, commands and submenus; comments and
    /// other keys ignored.
    #[test]
    fn command_menus_read_their_items() {
        let text = r##"// Command Menu Definition
"spectatormenu.res"
{
    "menuitem1" { "label" "#Valve_Close" "command" "spec_menu 0" }
    "menuitem2"
    {
        "label" "#Valve_Settings"
        "menuitem21" { "label" "#Valve_Overview_Names" "toggle" "overview_names" }
    }
    "type" "menu"
}"##;
        let root = parse(text);
        let root = root.items().iter().find_map(|(_, v)| matches!(v, Kv::Block(_)).then_some(v)).unwrap();
        let strings = HashMap::from([
            ("valve_close".to_string(), "Close".to_string()),
            ("valve_settings".to_string(), "Settings".to_string()),
        ]);
        let items = command_menu(root, &strings);
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].label.as_str(), items[0].command.as_deref()), ("Close", Some("spec_menu 0")));
        assert_eq!(items[1].label, "Settings");
        assert_eq!(items[1].items[0].toggle.as_deref(), Some("overview_names"));
        assert_eq!(items[1].items[0].label, "Valve_Overview_Names", "an unknown token shows its name");
    }
}
