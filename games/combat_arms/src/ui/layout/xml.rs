//! Reading `CBALAYOUT` files (specs/combat_arms/ui_system.md L1-L4): XML
//! in UTF-8 (a byte-order mark allowed), `Control` elements with a `Type`
//! and one `Property` child whose attributes are the widget's settings,
//! nested `Control`s as children. `ParentName` and the anchoring keys are
//! ignored; an unknown type fails the load. Our own layouts are in this
//! format (`data/ui/`); the game's sit in a keyed archive.
//!
//! The XML read is the subset layouts use: elements, attributes in single
//! or double quotes, the five named entities and numeric ones, comments,
//! the declaration and processing instructions; text between elements is
//! ignored.

use std::collections::HashMap;

use bevy::math::IVec2;

use super::{
    ButtonLook, GaugeLook, H_ALIGNS, ImageLook, Kind, Layout, LayoutError, ListLook, SOUNDS, Sound, TextLook,
    V_ALIGNS, text::EFFECTS, values,
};
use crate::client::canvas::{HAlign, VAlign, WidgetId};

/// An XML element: its name, attributes (in file order) and child
/// elements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
}

impl Element {
    /// An attribute by name (any case).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn err(at: usize, what: &str) -> LayoutError {
    LayoutError(format!("XML at byte {at}: {what}"))
}

fn unescape(s: &str, at: usize) -> Result<String, LayoutError> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let end = rest[i..].find(';').ok_or_else(|| err(at, "unterminated entity"))? + i;
        let name = &rest[i + 1..end];
        let c = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let n = if let Some(h) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    name.strip_prefix('#').and_then(|d| d.parse().ok())
                };
                n.and_then(char::from_u32).ok_or_else(|| err(at, "unknown entity"))?
            }
        };
        out.push(c);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Parse a document: its root element.
pub fn parse(text: &str) -> Result<Element, LayoutError> {
    let s = text.strip_prefix('\u{feff}').unwrap_or(text);
    let b = s.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Element> = vec![Element::default()];
    let skip_ws = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    let name_end = |i: usize| {
        let mut j = i;
        while j < b.len() && !b[j].is_ascii_whitespace() && !matches!(b[j], b'>' | b'/' | b'=') {
            j += 1;
        }
        j
    };
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if s[i..].starts_with("<!--") {
            i = s[i..].find("-->").ok_or_else(|| err(i, "unterminated comment"))? + i + 3;
        } else if s[i..].starts_with("<?") || s[i..].starts_with("<!") {
            i = s[i..].find('>').ok_or_else(|| err(i, "unterminated declaration"))? + i + 1;
        } else if s[i..].starts_with("</") {
            let end = name_end(i + 2);
            let name = &s[i + 2..end];
            let el = stack.pop().ok_or_else(|| err(i, "unbalanced close"))?;
            if el.name != name || stack.is_empty() {
                return Err(err(i, &format!("</{name}> closes <{}>", el.name)));
            }
            i = s[i..].find('>').ok_or_else(|| err(i, "unterminated tag"))? + i + 1;
            stack.last_mut().unwrap().children.push(el);
        } else {
            let end = name_end(i + 1);
            let mut el = Element {
                name: s[i + 1..end].to_string(),
                ..Default::default()
            };
            i = end;
            loop {
                skip_ws(&mut i);
                if i >= b.len() {
                    return Err(err(i, "unterminated tag"));
                }
                if b[i] == b'/' && b.get(i + 1) == Some(&b'>') {
                    i += 2;
                    stack.last_mut().unwrap().children.push(el);
                    break;
                }
                if b[i] == b'>' {
                    i += 1;
                    stack.push(el);
                    break;
                }
                let ne = name_end(i);
                let key = s[i..ne].to_string();
                i = ne;
                skip_ws(&mut i);
                if b.get(i) != Some(&b'=') {
                    return Err(err(i, &format!("attribute {key} has no value")));
                }
                i += 1;
                skip_ws(&mut i);
                let q = *b.get(i).ok_or_else(|| err(i, "attribute value"))?;
                if q != b'"' && q != b'\'' {
                    return Err(err(i, "attribute value not quoted"));
                }
                let close = s[i + 1..].find(q as char).ok_or_else(|| err(i, "unterminated value"))? + i + 1;
                el.attributes.push((key, unescape(&s[i + 1..close], i)?));
                i = close + 1;
            }
        }
    }
    if stack.len() != 1 {
        return Err(err(b.len(), "unclosed element"));
    }
    stack.pop().unwrap().children.into_iter().next().ok_or_else(|| err(0, "no root element"))
}

/// The widget types layouts know, lowercased (L2).
pub const TYPES: &[&str] = &[
    "window",
    "button",
    "simplestretchbutton",
    "checkbutton",
    "image",
    "simplestretchimage",
    "text",
    "buttontypetext",
    "scrollbar",
    "listctrl",
    "scrolllistctrl",
    "columnctrl",
    "combobox",
    "gauge",
    "sliderbar",
    "editctrl",
];

/// A `CBALAYOUT` document as a layout under the 1024 x 1024 root.
pub fn read(text: &str) -> Result<Layout, LayoutError> {
    let root = parse(text)?;
    if !root.name.eq_ignore_ascii_case("CBALAYOUT") {
        return Err(LayoutError(format!("root element <{}>, not <CBALAYOUT>", root.name)));
    }
    let mut layout = Layout::new();
    for c in root.children.iter().filter(|c| c.name.eq_ignore_ascii_case("Control")) {
        control(&mut layout, 0, c)?;
    }
    Ok(layout)
}

fn control(layout: &mut Layout, parent: WidgetId, el: &Element) -> Result<(), LayoutError> {
    let ty = el.attr("Type").unwrap_or("").to_lowercase();
    // Exact first, then the prefix rule (an empty type takes the first).
    let ty = if TYPES.contains(&ty.as_str()) {
        ty
    } else {
        TYPES
            .iter()
            .find(|t| t.starts_with(&ty))
            .map(|t| t.to_string())
            .ok_or_else(|| LayoutError(format!("control of unknown type {ty:?}")))?
    };
    let empty = Element::default();
    let p = el
        .children
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case("Property"))
        .unwrap_or(&empty);
    let a: HashMap<String, &str> = p
        .attributes
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.as_str()))
        .collect();
    let get = |k: &str| a.get(&k.to_lowercase()).copied();
    let name = get("Name").unwrap_or("").to_string();
    let scroll = ty == "scrollbar";
    let pair = |k: &str| get(k).map_or(IVec2::ZERO, values::pair);
    let (size, pos) = if scroll {
        (pair("ScrollSize"), pair("ScrollPosition"))
    } else {
        (pair("Size"), pair("Position"))
    };
    let flag = |k: &str, d: bool| get(k).map_or(d, values::boolean);
    let int = |k: &str| get(k).map_or(0, values::integer);
    let quad = |k: &str| get(k).map(values::quad);
    let sound = |k: &str, d: Sound| get(k).map_or(d, |v| values::enumeration(v, SOUNDS).unwrap_or(d));
    let image = || ImageLook {
        texture: get("Texture").unwrap_or("").to_string(),
        alpha: flag("EnableAlpha", false).then(|| int("AlphaValue") as u8),
        colour: flag("EnableColor", false).then(|| get("TextureColor").map_or([0; 4], values::colour)),
        uv: quad("CustomUV").or(Some([-1.0; 4])),
        stretch: quad("SimpleStretchValue"),
    };
    let button = || ButtonLook {
        prefix: get("ButtonTexture").unwrap_or("").to_string(),
        over: sound("OverSound", Sound::Change),
        click: sound("ClickSound", Sound::Select),
        stretch: quad("SimpleStretchValue"),
        check: (ty == "checkbutton").then(|| get("CheckButtonType").is_some_and(|v| v.eq_ignore_ascii_case("CHECK"))),
    };
    let text = |states: bool| TextLook {
        font: get("Font").unwrap_or("").to_string(),
        element: get("Element").unwrap_or("").to_string(),
        h: get("HorizontalAlign").map_or(HAlign::Default, |v| values::enumeration(v, H_ALIGNS).unwrap_or_default()),
        v: get("VerticalAlign").map_or(VAlign::Default, |v| values::enumeration(v, V_ALIGNS).unwrap_or_default()),
        effect: get("Effect").map_or_else(Default::default, |v| values::enumeration(v, EFFECTS).unwrap_or_default()),
        colour: values::colour_alpha(get("Normalcolor").unwrap_or(""), get("TextAlphaColor"), 255),
        stroke: values::colour_alpha(get("StrokeColor").unwrap_or("0,0,0"), get("StrokeAlphaColor"), 255),
        shadow: values::colour_alpha(get("ShadowColor").unwrap_or("0,0,0"), get("ShadowAlphaColor"), 255),
        wrap: int("WrapWidth"),
        states: states.then(|| {
            let c = |k: &str| get(k).map_or([0; 4], values::colour);
            [c("Overcolor"), c("Clickcolor"), c("Disablecolor")]
        }),
        set: None,
    };
    let list = |back: &str| ListLook {
        gap: int("ItemGap"),
        blank: int("Blank"),
        back: get(back).unwrap_or("").to_string(),
        auto_size: flag("AutoSize", false),
    };
    let kind = match ty.as_str() {
        "window" => Kind::Window(ImageLook {
            texture: get("Texture").unwrap_or("").to_string(),
            ..Default::default()
        }),
        "image" | "simplestretchimage" => Kind::Image(image()),
        "button" | "simplestretchbutton" | "checkbutton" => Kind::Button(button()),
        "text" => Kind::Text(text(false)),
        "buttontypetext" => Kind::Text(text(true)),
        "gauge" => Kind::Gauge(GaugeLook {
            frame: get("GaugeBackFrameTexture").unwrap_or("").to_string(),
            bar: get("GaugeBarTexture").unwrap_or("").to_string(),
            bar_at: pair("GaugeBarPosition"),
            max_range: int("GaugeBarMaxRange"),
            fill: int("GaugeBarWidth") as f64,
            bar_height: int("GaugeBarHeight"),
            frame_stretch: quad("GaugeBackTextureSimpleStretchValue"),
            bar_stretch: quad("GaugeBarTextureSimpleStretchValue"),
        }),
        "listctrl" | "scrolllistctrl" => Kind::List(list("BackTexture")),
        "columnctrl" => Kind::Column(list("BackTexture")),
        other => Kind::Other(other.to_string()),
    };
    let id = layout.add(parent, &name, pos, size, kind);
    {
        let n = layout.tree.get_mut(id);
        n.show = flag("Show", true);
        n.mouse = flag("MouseEvent", true);
        n.data.cmd = int("Cmd_id");
        n.data.texture_alpha_size = pair("TextureAlphaSize");
        n.data.scroller = get("Scroller")
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"))
            .map(str::to_string);
    }
    for c in el.children.iter().filter(|c| c.name.eq_ignore_ascii_case("Control")) {
        control(layout, id, c)?;
    }
    Ok(())
}
