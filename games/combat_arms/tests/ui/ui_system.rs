//! specs/combat_arms/ui_system.md's test cases, one test each (`t01_` is
//! test case 1), on the layout engine (`ui::layout`, Chimera's
//! `client::canvas`), the desk (`ui::desk`), the chat queue (`ui::chat`)
//! and the in-game menu (`ui::menu`), headless. Layout L1 is written in
//! the game's layout format and read with `layout::xml`; the stub fonts
//! T10 and T13 are monospaced as the spec says.

use std::collections::HashMap;

use bevy::{math::IVec2, prelude::*};
use chimera_client::client::{
    canvas::{Blink, Fade, Scale, gauge_width},
    front_end::MenuStatus,
};
use combat_arms::ui::{
    CaUi,
    chat::{ChatQueue, Message, PLAYER_LIFE, TEMPLATE_AT, height},
    desk::{Desk, Event, sounds},
    layout::{
        self, Ctx, Draw, Effect, Font, FontList, Kind, Layout, PxRect, Sound, Strings, TextDraw, list_rows, text,
        values, xml,
    },
    menu::{self, CaMenuStatePlugin, InGameMenu, MenuPointer},
};

/// L1 (the spec's table), in the layout format.
const L1: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<CBALAYOUT>
  <Control Type="window">
    <Property Name="Panel" ParentName="none" Size="200, 100" Position="100.50" Texture="blank"/>
    <Control Type="Image">
      <Property Name="Bg" Size="200, 100" Position="0,0" Texture="panel_bg"/>
    </Control>
    <Control Type="Text">
      <Property Name="Title" Position="100, 100" Font="T10" HorizontalAlign="HCENTER" VerticalAlign="VCENTER" Element="$t_title" Normalcolor="255.255.255"/>
    </Control>
    <Control Type="Text">
      <Property Name="Corner" Position="0,0" Font="T10" HorizontalAlign="RIGHT" VerticalAlign="BOTTOM" Element="AB" Normalcolor="255,255,255"/>
    </Control>
    <Control Type="Text">
      <Property Name="Plain" Position="10.5" Font="T10" HorizontalAlign="DEFAULT" VerticalAlign="DEFAULT" Element="Hi" Normalcolor="255,255,255"/>
    </Control>
    <Control Type="Button">
      <Property Name="Ok" Size="50, 20" Position="140, 70" ButtonTexture="ok_"/>
    </Control>
  </Control>
  <Control Type="Text">
    <Property Name="Lonely" Position="0,0" Font="T10" HorizontalAlign="HCENTER" VerticalAlign="VCENTER" Element="XY" Normalcolor="255,255,255"/>
  </Control>
</CBALAYOUT>
"#;

fn fonts() -> FontList {
    let mut f = FontList::default();
    for (name, height, average) in [("T10", 10, 6), ("T13", 13, 8)] {
        f.add(Font {
            name: name.into(),
            height,
            average,
            face: "bold".into(),
        });
    }
    f
}

fn strings() -> Strings {
    Strings::parse("t_title = \"ABCDE\"\n")
}

fn textures() -> HashMap<String, IVec2> {
    let mut t = HashMap::new();
    t.insert("panel_bg".to_string(), IVec2::new(256, 128));
    for s in ["A", "B", "C", "D"] {
        t.insert(format!("ok_{s}"), IVec2::new(50, 20));
    }
    t
}

struct World {
    fonts: FontList,
    strings: Strings,
    textures: HashMap<String, IVec2>,
}

impl World {
    fn new() -> Self {
        Self {
            fonts: fonts(),
            strings: strings(),
            textures: textures(),
        }
    }
    fn ctx(&self) -> Ctx<'_, HashMap<String, IVec2>> {
        Ctx {
            fonts: &self.fonts,
            strings: &self.strings,
            textures: &self.textures,
        }
    }
}

fn l1(screen: (f32, f32)) -> Layout {
    let mut l = xml::read(L1).unwrap();
    l.scale = Scale::of(Vec2::new(screen.0, screen.1), layout::REFERENCE);
    l
}

fn quad(l: &Layout, w: &World, name: &str) -> Option<PxRect> {
    let id = l.find(name).unwrap();
    l.draw(&w.ctx()).into_iter().find_map(|d| match d {
        Draw::Quad(i, q) if i == id => Some(q.cover),
        _ => None,
    })
}

fn text_of(l: &Layout, w: &World, name: &str) -> TextDraw {
    l.text_draw(l.find(name).unwrap(), &w.ctx()).unwrap()
}

fn r(x0: i32, x1: i32, y0: i32, y1: i32) -> PxRect {
    PxRect::new(x0, x1, y0, y1)
}

#[test]
fn t01_l1_at_1024x768() {
    let w = World::new();
    let l = l1((1024.0, 768.0));
    assert_eq!(quad(&l, &w, "Panel"), None, "E1: the blank texture draws no quad");
    let bg = l.find("Bg").unwrap();
    let q = l
        .draw(&w.ctx())
        .into_iter()
        .find_map(|d| match d {
            Draw::Quad(i, q) if i == bg => Some(q),
            _ => None,
        })
        .unwrap();
    assert_eq!(q.cover, r(100, 300, 50, 150));
    assert_eq!(q.uv, [0.0, 0.0, 200.0 / 256.0, 100.0 / 128.0]);
    let t = text_of(&l, &w, "Title");
    assert_eq!(t.lines[0].1, "ABCDE");
    assert_eq!(t.rect, r(185, 215, 95, 105));
    let t = text_of(&l, &w, "Corner");
    assert_eq!((t.lines[0].1.as_str(), t.rect), ("AB", r(288, 300, 140, 150)));
    let t = text_of(&l, &w, "Plain");
    assert_eq!((t.lines[0].1.as_str(), t.rect), ("Hi", r(110, 122, 55, 65)));
    assert_eq!(quad(&l, &w, "Ok"), Some(r(240, 290, 120, 140)));
    assert_eq!(l.hit_rect(l.find("Ok").unwrap(), &w.ctx()), r(240, 290, 120, 140));
    let t = text_of(&l, &w, "Lonely");
    assert_eq!((t.lines[0].1.as_str(), t.rect), ("XY", r(506, 518, 507, 517)));
}

#[test]
fn t02_l1_at_1920x1080() {
    let w = World::new();
    let l = l1((1920.0, 1080.0));
    assert_eq!(quad(&l, &w, "Bg"), Some(r(187, 562, 70, 211)));
    let t = text_of(&l, &w, "Title");
    assert_eq!((t.font.height, t.font.advance), (14, 11));
    assert_eq!(t.rect.size(), IVec2::new(55, 14));
    assert_eq!((t.reference_size, t.reference_at), (IVec2::new(29, 9), IVec2::new(186, 96)));
    assert_eq!(t.rect, r(348, 403, 135, 149));
    let t = text_of(&l, &w, "Corner");
    assert_eq!(t.rect.size(), IVec2::new(22, 14));
    assert_eq!((t.reference_size, t.reference_at), (IVec2::new(11, 9), IVec2::new(289, 141)));
    assert_eq!(t.rect, r(541, 563, 198, 212), "overhangs the background by 1 px right and down (quirk 4)");
    assert_eq!(text_of(&l, &w, "Plain").rect, r(206, 228, 77, 91));
    let ok = l.find("Ok").unwrap();
    assert_eq!(quad(&l, &w, "Ok"), Some(r(450, 543, 169, 197)));
    assert_eq!(l.hit_rect(ok, &w.ctx()), r(450, 543, 168, 196));
    let t = text_of(&l, &w, "Lonely");
    assert_eq!(t.reference_at, IVec2::new(507, 508));
    assert_eq!(t.rect, r(950, 972, 714, 728), "centred on the 1024-tall root (quirk 1)");
}

#[test]
fn t03_l1_at_800x600() {
    let w = World::new();
    let l = l1((800.0, 600.0));
    assert_eq!(quad(&l, &w, "Bg"), Some(r(78, 234, 39, 117)));
    let t = text_of(&l, &w, "Title");
    assert_eq!((t.font.height, t.font.advance), (7, 4));
    assert_eq!(t.rect.size(), IVec2::new(20, 7));
    assert_eq!((t.reference_size, t.reference_at), (IVec2::new(25, 8), IVec2::new(188, 96)));
    assert_eq!(t.rect, r(146, 166, 75, 82));
    let t = text_of(&l, &w, "Corner");
    assert_eq!(t.rect.size(), IVec2::new(8, 7));
    assert_eq!((t.reference_size, t.reference_at), (IVec2::new(10, 8), IVec2::new(290, 142)));
    assert_eq!(t.rect, r(226, 234, 110, 117));
    assert_eq!(text_of(&l, &w, "Plain").rect, r(85, 93, 42, 49));
    assert_eq!(quad(&l, &w, "Ok"), Some(r(187, 226, 94, 109)));
    assert_eq!(text_of(&l, &w, "Lonely").rect, r(396, 404, 396, 403));
}

#[test]
fn t04_text_effects() {
    let w = World::new();
    let copies = |effect: Effect| {
        let mut l = l1((1024.0, 768.0));
        let id = l.find("Plain").unwrap();
        if let Kind::Text(t) = &mut l.widget_mut(id).kind {
            t.effect = effect;
        }
        let t = l.text_draw(id, &w.ctx()).unwrap();
        let at = t.lines[0].0;
        assert_eq!(at, IVec2::new(110, 55), "the text itself, drawn last");
        t.copies.iter().map(|o| at + *o).map(|p| (p.x, p.y)).collect::<Vec<_>>()
    };
    let stroke = copies(Effect::Stroke);
    assert_eq!(
        stroke,
        [(110, 55), (109, 55), (111, 55), (110, 54), (110, 56), (111, 54), (109, 56), (111, 56)]
    );
    assert!(!stroke.contains(&(109, 54)));
    assert_eq!(copies(Effect::Shadow), [(111, 56)]);
    assert_eq!(
        copies(Effect::Stroke2x),
        [(110, 55), (108, 55), (112, 55), (110, 53), (110, 57), (112, 53), (108, 57), (112, 57)]
    );
}

#[test]
fn t05_value_parsing() {
    assert_eq!(values::pair("340.80"), IVec2::new(340, 80));
    assert_eq!(values::pair("-7.717"), IVec2::new(-7, 717));
    assert_eq!(values::pair("5"), IVec2::new(1, 1));
    assert_eq!(values::pair("1,2,3"), IVec2::new(1, 2));
    assert_eq!(values::colour_alpha("124.190.221", Some("150"), 255), [124, 190, 221, 150]);
    assert_eq!(values::colour("1,2"), [0, 0, 0, 0]);
    assert_eq!(values::colour("1,2,3,4"), [0, 0, 0, 0]);
    assert_eq!(values::quad(" 0.25, 0, 0.5, 1"), [0.25, 0.0, 0.5, 1.0]);
    assert!(values::boolean("on"));
    assert!(!values::boolean("1"));
}

#[test]
fn t06_string_resolution() {
    let s = strings();
    assert_eq!(s.resolve("$t_title"), "ABCDE");
    assert_eq!(s.resolve("t_title"), "ABCDE");
    assert_eq!(s.resolve("$$T_TITLE"), "ABCDE");
    assert_eq!(s.resolve("$no_such_id"), "$no_such_id");
    assert_eq!(s.resolve("AB"), "AB");
    let t = Strings::parse("[Section]\nstray words\nx = \"\"\nt_title = \"ABCDE\"\nt_title_r = \"Rent\"\nq = \"a\"\"b\"\n");
    assert_eq!(t.resolve("$x"), "");
    assert_eq!(text::measure(&t.resolve("$x"), &fonts().fonts["T10"], 0, Scale::ONE).px.x, 0);
    assert_eq!(t.values.len(), 4, "lines without = are ignored");
    assert_eq!(t.resolve("$q"), "a\"b");
    let mut rental = t.clone();
    rental.suffix = Some("_r".into());
    assert_eq!(rental.resolve("$t_title"), "Rent");
}

#[test]
fn t07_wrapping() {
    let t10 = fonts().fonts["T10"].clone();
    let m = text::measure("AAA BBB CCC", &t10, 30, Scale::ONE);
    assert_eq!(m.lines, ["AAA ", "BBB ", "CCC"]);
    assert_eq!(m.reference, IVec2::new(24, 33));
    assert_eq!(text::measure("ABCDEFGH", &t10, 30, Scale::ONE).lines, ["ABCDE", "FGH"]);
    assert_eq!(text::measure("ABC", &t10, 4, Scale::ONE).lines, ["A", "B", "C"], "raised to the advance");
    let m = text::measure("A\\nB", &t10, 0, Scale::ONE);
    assert_eq!((m.lines.len(), m.px.x), (1, 24), "unwrapped: a literal backslash and n");
    assert_eq!(text::measure("A\\nB", &t10, 30, Scale::ONE).lines, ["A", "B"]);
}

#[test]
fn t08_wrapped_alignment() {
    let w = World::new();
    let mut l = l1((1024.0, 768.0));
    let panel = l.find("Panel").unwrap();
    let id = l.add(
        panel,
        "Wrapped",
        IVec2::ZERO,
        IVec2::ZERO,
        Kind::Text(layout::TextLook {
            font: "T10".into(),
            element: "AAA BBB CCC".into(),
            h: chimera_client::client::canvas::HAlign::Centre,
            wrap: 30,
            colour: [255; 4],
            ..Default::default()
        }),
    );
    let t = l.text_draw(id, &w.ctx()).unwrap();
    assert_eq!(t.lines[0].0.x, 188);
    assert_eq!(t.lines[2].0.x, 191);
    assert_eq!(t.lines[1].0.y - t.lines[0].0.y, 11);
    assert_eq!(t.lines[2].0.y - t.lines[1].0.y, 11);
}

fn chat_at(screen: (f32, f32), n: usize) -> ChatQueue {
    let t13 = fonts().fonts["T13"].clone();
    let s = Scale::of(Vec2::new(screen.0, screen.1), layout::REFERENCE);
    let mut q = ChatQueue::default();
    for i in 0..n {
        q.push(Message {
            text: format!("message {i}"),
            born: 0.0,
            life: PLAYER_LIFE,
            height: height(&format!("message {i}"), &t13, s),
            ..Default::default()
        });
    }
    q
}

#[test]
fn t09_chat_queue_at_768() {
    let q = chat_at((1024.0, 768.0), 10);
    assert_eq!(q.messages.len(), 9);
    assert_eq!(q.messages[0].text, "message 1", "the first is gone");
    let want: Vec<i32> = (0..9).map(|i| 525 + 19 * i).collect();
    assert_eq!(q.tops(), want);
}

#[test]
fn t10_chat_queue_at_1080() {
    let q = chat_at((1920.0, 1080.0), 10);
    assert_eq!(q.messages[0].height, 14);
    assert_eq!(q.messages.len(), 10);
    let want: Vec<i32> = (0..10).map(|i| 525 + 18 * i).collect();
    assert_eq!(q.tops(), want);
}

#[test]
fn t11_chat_expiry_removes_the_oldest() {
    let mut q = ChatQueue::default();
    for (text, born, life) in [("player", 0.0, 10000.0), ("system", 1000.0, 5000.0)] {
        q.push(Message {
            text: text.into(),
            born,
            life,
            height: 15,
            ..Default::default()
        });
    }
    q.frame(6000.0);
    assert_eq!(q.messages.len(), 2);
    q.frame(6001.0);
    assert_eq!(q.messages.len(), 1);
    assert_eq!((q.messages[0].text.as_str(), q.tops()[0]), ("system", TEMPLATE_AT.1));
    q.frame(6002.0);
    assert!(q.messages.is_empty());
}

#[test]
fn t12_gauge() {
    assert_eq!(gauge_width(50.0, 120), Some(60));
    assert_eq!(gauge_width(99.9, 120), Some(119));
    assert_eq!(gauge_width(100.0, 120), Some(120));
    assert_eq!(gauge_width(0.0, 120), None);
    assert_eq!(gauge_width(-5.0, 120), None);
}

fn button_layout(prefix: &str, size: IVec2) -> Layout {
    let mut l = Layout::new();
    l.add(
        0,
        "B",
        IVec2::new(10, 10),
        size,
        Kind::Button(layout::ButtonLook {
            prefix: prefix.into(),
            over: Sound::Change,
            click: Sound::Select,
            ..Default::default()
        }),
    );
    l
}

#[test]
fn t13_button_textures() {
    let mut tex: HashMap<String, IVec2> = HashMap::new();
    tex.insert("ab_A".into(), IVec2::new(64, 32));
    tex.insert("ab_B".into(), IVec2::new(64, 32));
    let w = World {
        textures: tex.clone(),
        ..World::new()
    };
    let mut l = button_layout("ab_", IVec2::new(64, 32));
    let shown = |l: &Layout| match &l.draw(&w.ctx())[0] {
        Draw::Quad(_, q) => q.texture.clone(),
        d => panic!("{d:?}"),
    };
    l.widget_mut(1).hover = true;
    assert_eq!(shown(&l), "ab_B");
    l.widget_mut(1).held = true;
    assert_eq!(shown(&l), "ab_A", "pressed falls back to A");
    l.widget_mut(1).enabled = false;
    assert_eq!(shown(&l), "ab_A", "disabled falls back to A");
    // No A: 0 x 0, never fires.
    let l = button_layout("none_", IVec2::new(64, 32));
    assert_eq!(l.size_of(1, &tex), IVec2::ZERO);
    assert!(l.draw(&w.ctx()).is_empty());
    assert_eq!(l.click_target(IVec2::new(20, 20), &w.ctx()), None);
    // Size 0 takes A's.
    let l = button_layout("ab_", IVec2::ZERO);
    assert_eq!(l.size_of(1, &tex), IVec2::new(64, 32));
    assert_eq!(l.click_target(IVec2::new(70, 40), &w.ctx()), Some(1));
}

fn desk_of(l: Layout) -> (Desk, usize) {
    let mut d = Desk::default();
    let i = d.add("screen", l, true);
    d.show(i, false);
    (d, i)
}

#[test]
fn t14_button_behaviour() {
    let w = World::new();
    let (mut d, _) = desk_of(l1((1024.0, 768.0)));
    let ctx = w.ctx();
    d.pointer_moved(IVec2::new(250, 125), &ctx);
    assert_eq!(sounds(&d.take_events()), [Sound::Change], "entering plays the hover sound");
    d.pointer_moved(IVec2::new(260, 130), &ctx);
    assert!(sounds(&d.take_events()).is_empty(), "moving within: nothing more");
    d.pointer_moved(IVec2::new(10, 10), &ctx);
    d.pointer_moved(IVec2::new(250, 125), &ctx);
    assert_eq!(sounds(&d.take_events()), [Sound::Change], "re-entering plays it again");
    d.left_down(IVec2::new(250, 125), &ctx);
    let e = d.take_events();
    assert_eq!(
        e,
        [
            Event::Sound(Sound::Select),
            Event::Fired {
                tree: 0,
                name: "Ok".into(),
                cmd: 0
            }
        ],
        "fires on the press, with the click sound, before any release"
    );
    d.left_up();
    // IS_NONE: silent; IS_ITEM_SELECT: the item-select sound.
    let mut l = l1((1024.0, 768.0));
    let ok = l.find("Ok").unwrap();
    if let Kind::Button(b) = &mut l.widget_mut(ok).kind {
        b.over = values::enumeration("IS_NONE", layout::SOUNDS).unwrap();
        b.click = values::enumeration("IS_ITEM_SELECT", layout::SOUNDS).unwrap();
    }
    let (mut d, _) = desk_of(l);
    d.pointer_moved(IVec2::new(250, 125), &ctx);
    assert!(sounds(&d.take_events()).is_empty());
    d.left_down(IVec2::new(250, 125), &ctx);
    assert_eq!(sounds(&d.take_events()), [Sound::ItemSelect]);
    assert_eq!(Sound::ItemSelect.value(), 3);
}

/// Two overlapping buttons A then B in a window.
fn overlapping(window_mouse: bool) -> Layout {
    let mut l = Layout::new();
    let win = l.add(0, "Win", IVec2::ZERO, IVec2::new(300, 300), Kind::Window(Default::default()));
    l.tree.get_mut(win).mouse = window_mouse;
    for (name, at) in [("A", 10), ("B", 30)] {
        let b = l.add(
            win,
            name,
            IVec2::splat(at),
            IVec2::new(50, 50),
            Kind::Button(layout::ButtonLook {
                prefix: "ok_".into(),
                ..Default::default()
            }),
        );
        l.widget_mut(b).cmd = at;
    }
    l
}

fn fired(d: &mut Desk) -> Option<String> {
    d.take_events().into_iter().find_map(|e| match e {
        Event::Fired { name, .. } => Some(name),
        _ => None,
    })
}

#[test]
fn t15_hit_testing() {
    let w = World::new();
    let ctx = w.ctx();
    let p = IVec2::new(40, 40);
    let (mut d, _) = desk_of(overlapping(true));
    d.left_down(p, &ctx);
    assert_eq!(fired(&mut d).as_deref(), Some("B"), "the later sibling");
    // Under a window with MouseEvent OFF: neither; the click falls to the
    // next tree down.
    let mut d = Desk::default();
    let below = d.add("below", button_layout("ok_", IVec2::new(100, 100)), true);
    let above = d.add("above", overlapping(false), true);
    d.show(below, false);
    d.show(above, false);
    d.left_down(p, &ctx);
    assert_eq!(fired(&mut d).as_deref(), Some("B"));
    assert_eq!(d.pressed.map(|(t, _)| t), Some(below));
    // A hidden parent: its children can't be clicked.
    let mut l = overlapping(true);
    l.tree.get_mut(1).show = false;
    let (mut d, _) = desk_of(l);
    d.left_down(p, &ctx);
    assert_eq!(fired(&mut d), None);
}

#[test]
fn t16_modal_popup() {
    let w = World::new();
    let ctx = w.ctx();
    let mut d = Desk::default();
    let screen = d.add("screen", button_layout("ok_", IVec2::new(50, 20)), true);
    d.show(screen, false);
    let mut popup = Layout::new();
    let b = popup.add(0, "Box", IVec2::ZERO, IVec2::new(220, 145), Kind::Window(Default::default()));
    popup.centre(b, IVec2::new(1024, 768));
    let popup = d.add("popup", popup, true);
    d.show(popup, true);
    let outside = IVec2::new(20, 15);
    d.left_down(outside, &ctx);
    assert_eq!(fired(&mut d), None, "the modal popup swallows it");
    d.hide(popup);
    d.left_down(outside, &ctx);
    assert_eq!(fired(&mut d).as_deref(), Some("B"));
}

#[test]
fn t17_in_game_menu_centring() {
    let ui = CaUi::default();
    let mut m = InGameMenu::new(&ui);
    for (screen, at) in [((1024, 768), (402, 312)), ((1920, 1080), (850, 468)), ((1366, 768), (573, 312))] {
        m.screen = IVec2::new(screen.0, screen.1);
        m.place();
        let l = m.layout();
        let id = l.find(menu::BOX).unwrap();
        assert_eq!(l.tree.abs_pos(id), IVec2::new(at.0, at.1), "{screen:?}");
        assert_eq!(l.tree.get(id).size, IVec2::new(220, 145));
        // Drawn 1:1: 220 px wide at every resolution.
        let q = l
            .draw(&ui.ctx())
            .into_iter()
            .find_map(|d| match d {
                Draw::Quad(i, q) if i == id => Some(q.cover),
                _ => None,
            })
            .unwrap();
        assert_eq!(q, PxRect::new(at.0, at.0 + 220, at.1, at.1 + 145));
    }
}

#[test]
fn t18_scoreboard_rows() {
    let rows = list_rows(192, 0, 0, &[25; 9]);
    assert_eq!(&rows[..5], [192, 217, 242, 267, 292]);
    assert_eq!(rows[8], 392, "the ninth overlaps the Bravo list");
}

#[test]
fn t19_fades() {
    let fade_in = Fade {
        start: 0.0,
        length: 2000.0,
        fade_in: true,
    };
    assert_eq!(fade_in.alpha(500.0), Some(63));
    assert_eq!(fade_in.alpha(1000.0), Some(127));
    assert_eq!(fade_in.alpha(2000.0), Some(255));
    assert!(fade_in.done(2000.0));
    let fade_out = Fade { fade_in: false, ..fade_in };
    assert_eq!(fade_out.alpha(1000.0), Some(128));
    assert_eq!(fade_out.alpha(2000.0), None, "hidden");
}

#[test]
fn t20_blink() {
    let mut b = Blink::new(0.0, 1000.0, 300.0, 2);
    let mut shown = Vec::new();
    let mut t = 0;
    while t <= 2500 {
        shown.push((t, b.frame(t as f64)));
        t += 10;
    }
    let at = |t: i32| shown.iter().find(|(s, _)| *s == t).unwrap().1;
    for t in (0..=700).step_by(10) {
        assert!(at(t), "{t}");
    }
    for t in (710..1000).step_by(10) {
        assert!(!at(t), "{t}");
    }
    for t in (1000..=1700).step_by(10) {
        assert!(at(t), "{t}");
    }
    assert!(!at(1710), "hidden on the first frame after 1700");
    for t in (1720..=2500).step_by(10) {
        assert!(at(t), "{t}: over");
    }
    assert!(b.over());
}

#[test]
fn t21_font_scaling() {
    let t13 = fonts().fonts["T13"].clone();
    let m = |w: f32, h: f32| text::measure("HELLO", &t13, 0, Scale::of(Vec2::new(w, h), layout::REFERENCE));
    assert_eq!(m(1024.0, 768.0).px, IVec2::new(40, 13));
    let wide = m(1920.0, 1080.0);
    assert_eq!((wide.px, wide.font.advance, wide.reference.x), (IVec2::new(75, 18), 15, 40));
    let tall = m(1280.0, 1024.0);
    assert_eq!((tall.px, tall.font.advance, tall.reference.x), (IVec2::new(50, 17), 10, 40));
}

#[test]
fn t22_missing_font_fails_the_load() {
    let doc = r#"<CBALAYOUT><Control Type="text"><Property Name="Big" Font="bold 99" Element="x"/></Control></CBALAYOUT>"#;
    let l = xml::read(doc).unwrap();
    let e = l.check_fonts(&FontList::ours()).unwrap_err();
    assert!(e.0.contains("Big") && e.0.contains("bold 99"), "{e}");
    // Our list has the spec's names.
    let ours = FontList::ours();
    for name in ["bold 11", "bold 60", "Light 15", "number 40"] {
        assert!(ours.get(name).is_some(), "{name}");
    }
    assert!(ours.get("light 15").is_none(), "names match case-sensitively");
}

/// The in-game menu: Esc opens it (the shell sees it open), the pointer
/// hovers and presses its buttons, Cancel closes it; Esc while the console
/// is open doesn't.
#[test]
fn in_game_menu_opens_with_esc_and_cancel_closes_it() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(CaMenuStatePlugin);
    app.update();
    let press = |app: &mut App, k: KeyCode| {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(k);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(k);
        keys.clear();
    };
    assert!(!app.world().resource::<MenuStatus>().open);
    press(&mut app, KeyCode::Escape);
    assert!(app.world().resource::<InGameMenu>().open);
    app.update();
    assert!(app.world().resource::<MenuStatus>().open && app.world().resource::<MenuStatus>().in_game);
    // Cancel's centre at 1024 x 768: box at (402, 312), button at (12, 111)
    // in it, 196 x 24.
    let cancel = IVec2::new(402 + 12 + 98, 312 + 111 + 12);
    app.world_mut().resource_mut::<MenuPointer>().0 = Some(cancel);
    app.update();
    assert_eq!(app.world().resource::<InGameMenu>().played, [Sound::Change]);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    let m = app.world().resource::<InGameMenu>();
    assert_eq!(m.fired, ["Cancel"]);
    assert_eq!(m.played, [Sound::Change, Sound::Select]);
    assert!(!m.open);
    app.update();
    assert!(!app.world().resource::<MenuStatus>().open);
    // The disabled buttons hit but don't fire.
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        keys.release(MouseButton::Left);
        keys.clear();
    }
    press(&mut app, KeyCode::Escape);
    let report = IVec2::new(402 + 50, 312 + 55 + 12);
    app.world_mut().resource_mut::<MenuPointer>().0 = Some(report);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<InGameMenu>().fired, ["Cancel"]);
    assert!(app.world().resource::<InGameMenu>().open);
}

/// The menu's layout is drawn: its labels from our strings, centred on
/// their buttons.
#[test]
fn in_game_menu_draws_its_labels() {
    let ui = CaUi::default();
    let m = InGameMenu::new(&ui);
    let texts: Vec<TextDraw> = m
        .layout()
        .draw(&ui.ctx())
        .into_iter()
        .filter_map(|d| match d {
            Draw::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    let names: Vec<&str> = texts.iter().map(|t| t.lines[0].1.as_str()).collect();
    assert_eq!(names, ["MENU", "Leave Game", "Report User", "Start Recording", "Cancel"]);
    let cancel = &texts[4];
    // Centred in the 196 x 24 button at (414, 423).
    let mid = cancel.rect.x0 + cancel.rect.size().x / 2;
    assert!((mid - (414 + 98)).abs() <= 1, "{:?}", cancel.rect);
    // The disabled ones in the disabled colour.
    assert_eq!(texts[2].colour, [110, 110, 110, 255]);
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): test cases 26 and 27 (23-25, 32: score.rs;
// 28-31: bound.rs)

/// The shell's input for the local player each frame, as its bindings
/// would write it (before the popup rule).
#[derive(Resource, Clone, Default)]
struct Keys(chimera_core::core::Intent);

fn write_keys(keys: Res<Keys>, mut player: Query<&mut chimera_core::core::Intent, With<chimera_core::core::LocalPlayer>>) {
    for mut i in &mut player {
        *i = keys.0.clone();
    }
}

/// 26. Input under the Esc menu (and the chat input): no movement, look or
/// fire from the next frame, a held crouch stays down, the clock runs;
/// closed, walking resumes.
#[test]
fn t26_input_under_the_esc_menu() {
    use chimera_core::core::{Intent, LocalPlayer, SimClock};
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    for chat in [false, true] {
        let mut sim = Sim::with(combat_arms::harness::CaSim, |app| {
            app.add_plugins((GreyboxMapPlugin, CaMenuStatePlugin))
                .init_resource::<Keys>()
                .add_systems(
                    RunFixedMainLoop,
                    write_keys.in_set(chimera_client::client::input::LocalInputSystems::Write),
                );
        });
        let me = sim.spawn_character(chimera_world::greybox::SPAWNS[0], combat_arms::game::movement::ID);
        sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
        sim.seconds(0.5);
        let held = Intent {
            move_axis: Vec2::Y,
            crouch: true,
            ..default()
        };
        sim.app.world_mut().resource_mut::<Keys>().0 = held.clone();
        sim.ticks(5);
        let open = |sim: &mut Sim, on: bool| {
            if chat {
                sim.app.world_mut().insert_resource(chimera_client::client::chat::ChatInput {
                    open: on.then_some(false),
                    ..Default::default()
                });
            } else {
                sim.app.world_mut().resource_mut::<InGameMenu>().set_open(on);
            }
        };
        open(&mut sim, true);
        sim.ticks(1);
        let clock = sim.app.world().resource::<SimClock>().now;
        // Keys still held, the mouse turning, fire pressed.
        sim.app.world_mut().resource_mut::<Keys>().0 = Intent {
            yaw: 1.0,
            fire: true,
            sprint: true,
            ..held.clone()
        };
        sim.ticks(3);
        let i = sim.app.world().get::<Intent>(me).unwrap().clone();
        assert_eq!((i.move_axis, i.fire, i.sprint), (Vec2::ZERO, false, false), "chat {chat}");
        assert!(i.crouch, "the crouch stays down");
        assert_eq!(i.yaw, 0.0, "mouse look ignored");
        assert!(sim.app.world().resource::<SimClock>().now > clock, "the clock runs");
        // Closed with forward still held: walking on the next frame.
        open(&mut sim, false);
        sim.app.world_mut().resource_mut::<Keys>().0 = held.clone();
        sim.ticks(1);
        assert_eq!(sim.app.world().get::<Intent>(me).unwrap().move_axis, Vec2::Y);
    }
}

/// The game's in-game menu layout, as the install has it (the controls
/// code binds by name, and an options button it doesn't).
const INSTALL_MENU: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<CBALAYOUT>
  <Control Type="window">
    <Property Name="exit_popup" Size="220,200" Position="0,0" Texture="igm_back"/>
    <Control Type="button">
      <Property Name="button_lobby" Size="196,24" Position="12,27" ButtonTexture="igm_button_" Cmd_id="4"/>
    </Control>
    <Control Type="button">
      <Property Name="button_record" Size="196,24" Position="12,55" ButtonTexture="igm_button_" Cmd_id="1"/>
      <Control Type="buttontypetext">
        <Property Name="btn_text_record" Font="bold 12" Element="sample" HorizontalAlign="HCENTER" VerticalAlign="VCENTER" Normalcolor="210,210,210"/>
      </Control>
    </Control>
    <Control Type="button">
      <Property Name="button_option" Size="196,24" Position="12,83" ButtonTexture="igm_button_" Cmd_id="1"/>
    </Control>
    <Control Type="button">
      <Property Name="button_cancel" Size="196,24" Position="12,111" ButtonTexture="igm_button_" Cmd_id="2"/>
    </Control>
    <Control Type="button">
      <Property Name="button_exit_title" Size="20,20" Position="196,2" ButtonTexture="igm_button_" Cmd_id="1"/>
    </Control>
  </Control>
</CBALAYOUT>
"#;

/// 27. The Esc menu's keys and buttons: Esc closes it, other keys don't;
/// `button_cancel` and `button_exit_title` close it; the unbound options
/// button looks normal and does nothing; the record label names the
/// record key.
#[test]
fn t27_esc_menu_keys_and_buttons() {
    let install = combat_arms::ui::install::InstallUi {
        menu: Some(xml::read(INSTALL_MENU).unwrap()),
        ..Default::default()
    };
    let ui = CaUi::with_install(std::sync::Arc::new(install));
    let mut app = App::new();
    app.insert_resource(ui.clone());
    app.add_plugins(MinimalPlugins).add_plugins(CaMenuStatePlugin);
    app.update();
    let press = |app: &mut App, k: KeyCode| {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(k);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(k);
        keys.clear();
    };
    let open = |app: &App| app.world().resource::<InGameMenu>().open;
    press(&mut app, KeyCode::Escape);
    assert!(open(&app));
    for k in [KeyCode::Enter, KeyCode::KeyY] {
        press(&mut app, k);
        assert!(open(&app), "{k:?} swallowed");
    }
    press(&mut app, KeyCode::Escape);
    assert!(!open(&app), "Esc closes it");
    // The options button: enabled (its normal look), no command.
    let m = app.world().resource::<InGameMenu>().clone();
    let option = m.layout().find("button_option").unwrap();
    assert!(m.layout().widget(option).enabled);
    assert_eq!(m.layout().widget(option).cmd, menu::CMD_NONE);
    // The record label: `$button_record_start` with the key's name.
    let label = m.layout().find(menu::RECORD_LABEL).unwrap();
    let set = m.layout().widget(label).text().unwrap().set.clone();
    assert_eq!(set, Some(menu::record_label(&ui.strings, "", false)));
    assert_eq!(menu::record_label(&ui.strings, "F8", false), "Record (F8)");
    assert_eq!(menu::record_label(&ui.strings, "F8", true), "Stop recording (F8)");
    // Clicks: the options button leaves it open; cancel and the title
    // bar's close button close it.
    let click = |app: &mut App, name: &str| {
        let m = app.world().resource::<InGameMenu>();
        let id = m.layout().find(name).unwrap();
        let r = m.layout().hit_rect(id, &ui.ctx());
        let centre = IVec2::new((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2);
        app.world_mut().resource_mut::<MenuPointer>().0 = Some(centre);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.update();
        let mut b = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        b.release(MouseButton::Left);
        b.clear();
        app.update();
    };
    for (name, closes) in [("button_option", false), ("button_cancel", true), ("button_exit_title", true)] {
        press(&mut app, KeyCode::Escape);
        assert!(open(&app));
        click(&mut app, name);
        assert_eq!(open(&app), !closes, "{name}");
        if open(&app) {
            press(&mut app, KeyCode::Escape);
        }
    }
}
