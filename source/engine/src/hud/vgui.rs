//! Source's VGUI layouts, for any Source game's menus and panels (lift
//! S7 of docs/plans/active/hl2dm.md): the `.res` reader is generic Source
//! VGUI: KeyValues blocks per control (`ControlName`, `fieldName`,
//! `xpos`/`ypos` with `r`/`c` anchors, `wide`/`tall`, `labelText` with
//! `#token` strings and `&` hotkeys, colour overrides), with `#base` files
//! merged under the file's own keys, into the neutral `map::hud::UiLayout`;
//! and the client scheme's text fonts (`fonts`).

use std::collections::HashMap;

use chimera_world::map::hud::{HudCoord, UiAlign, UiControl, UiFontSize, UiKind, UiLayout};
use source_formats::keyvalues::{Kv, parse_res as parse};

/// Read a `.res` file's root block with its `#base` files merged in (paths
/// relative to the file's folder; the file's own keys win).
pub fn read_res(read: &mut dyn FnMut(&str) -> Option<String>, path: &str) -> Option<Kv> {
    fn go(read: &mut dyn FnMut(&str) -> Option<String>, path: &str, depth: usize) -> Option<Kv> {
        let kv = parse(&read(path)?);
        let mut root = None;
        let mut bases = Vec::new();
        for (k, v) in kv.items() {
            match v {
                Kv::Value(b) if k.eq_ignore_ascii_case("#base") => bases.push(b.clone()),
                Kv::Block(_) if root.is_none() => root = Some(v.clone()),
                _ => {}
            }
        }
        let mut root = root.unwrap_or(Kv::Block(Vec::new()));
        if depth < 4 {
            let dir = path.rsplit_once(['/', '\\']).map(|(d, _)| d);
            for b in bases {
                let base_path = match dir {
                    Some(d) => format!("{d}/{b}"),
                    None => b,
                };
                if let Some(base) = go(read, &base_path, depth + 1) {
                    root = merge(base, root);
                }
            }
        }
        Some(root)
    }
    go(read, path, 0)
}

/// `over` laid on `base`: a key in both takes `over`'s value (blocks merged
/// key by key); keys only in `over` are added after `base`'s.
pub fn merge(base: Kv, over: Kv) -> Kv {
    let (Kv::Block(mut items), Kv::Block(over)) = (base, over.clone()) else {
        return over;
    };
    for (k, v) in over {
        match items.iter_mut().find(|(bk, _)| bk.eq_ignore_ascii_case(&k)) {
            Some((_, slot)) => *slot = merge(slot.clone(), v),
            None => items.push((k, v)),
        }
    }
    Kv::Block(items)
}

/// Text with VGUI's hotkey marker: `&1 PISTOLS` -> ("1 PISTOLS", '1');
/// `&&` is a literal ampersand.
pub fn hotkey(text: &str) -> (String, Option<char>) {
    let mut out = String::new();
    let mut key = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('&') => {
                out.push('&');
                chars.next();
            }
            Some(k) if key.is_none() => key = Some(k.to_ascii_lowercase()),
            _ => {}
        }
    }
    (out, key)
}

fn rgba(text: &str) -> Option<[u8; 4]> {
    let v: Vec<u8> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    match v.as_slice() {
        [r, g, b] => Some([*r, *g, *b, 255]),
        [r, g, b, a] => Some([*r, *g, *b, *a]),
        _ => None,
    }
}

fn kind(control: &str) -> UiKind {
    let c = control.to_lowercase();
    match c.as_str() {
        "label" => UiKind::Label,
        "button" | "mouseoverpanelbutton" | "commandbutton" => UiKind::Button,
        "imagepanel" | "scalableimagepanel" => UiKind::Image,
        "richtext" => UiKind::RichText,
        "panel" | "editablepanel" => UiKind::Panel,
        "divider" => UiKind::Divider,
        _ if c == "frame"
            || c.ends_with("menu")
            || c.ends_with("menu_ter")
            || c.ends_with("menu_ct")
            || c.ends_with("subpanel") =>
        {
            UiKind::Frame
        }
        _ => UiKind::Other(control.to_string()),
    }
}

/// A layout from a `.res` root block: `strings` localises `#token` text
/// (lower-case keys), `colors` names scheme colours, `image` turns an image
/// path into a sprite key (None: not drawn).
pub fn layout(
    root: &Kv,
    strings: &HashMap<String, String>,
    colors: &HashMap<String, [u8; 4]>,
    image: &mut dyn FnMut(&str) -> Option<String>,
) -> UiLayout {
    let color = |v: Option<&str>| v.and_then(|v| rgba(v).or_else(|| colors.get(v.trim()).copied()));
    let mut controls = Vec::new();
    for (key, c) in root.items() {
        if !matches!(c, Kv::Block(_)) {
            continue;
        }
        let s = |k: &str| c.str(k);
        let num = |k: &str| s(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
        let flag = |k: &str, default: bool| s(k).map_or(default, |v| v.trim() != "0");
        let raw = s("labelText").unwrap_or_default();
        let text = match raw.strip_prefix('#') {
            Some(token) => strings
                .get(&token.to_lowercase())
                .cloned()
                .unwrap_or_else(|| raw.to_string()),
            None => raw.to_string(),
        };
        let (text, hotkey) = hotkey(&text);
        let mut control = UiControl::new(
            s("fieldName").unwrap_or(key),
            kind(s("ControlName").unwrap_or_default()),
            0.0,
            0.0,
            num("wide"),
            num("tall"),
        );
        control.x = s("xpos").and_then(HudCoord::parse).unwrap_or(HudCoord::Start(0.0));
        control.y = s("ypos").and_then(HudCoord::parse).unwrap_or(HudCoord::Start(0.0));
        control.z = num("zpos") as i32;
        control.visible = flag("visible", true);
        control.enabled = flag("enabled", true);
        control.text = text;
        control.hotkey = hotkey;
        control.command = s("command").map(str::to_string).filter(|c| !c.is_empty());
        control.font = s("font").map(str::to_string);
        control.align = UiAlign::parse(s("textAlignment").unwrap_or_default());
        control.fg = color(s("fgcolor_override").or(s("fgcolor")));
        control.bg = color(s("bgcolor_override").or(s("bgcolor")));
        control.fill = color(s("fillColor"));
        control.image = s("image").filter(|i| !i.is_empty()).and_then(&mut *image);
        control.dull = flag("dulltext", false);
        control.bright = flag("brighttext", false);
        control.wrap = flag("wrap", false);
        control.keys = c
            .items()
            .iter()
            .filter_map(|(k, v)| match v {
                Kv::Value(v) => Some((k.to_lowercase(), v.clone())),
                Kv::Block(_) => None,
            })
            .collect();
        controls.push(control);
    }
    UiLayout { controls }
}

/// The scheme's fonts: every size entry of every font, in order.
pub fn fonts(scheme: &Kv) -> HashMap<String, Vec<UiFontSize>> {
    super::look::scheme_fonts(scheme).into_iter().collect()
}

