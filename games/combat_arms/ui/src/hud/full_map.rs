//! The full-screen map, shown while M is held (specs/combat_arms/
//! radar_minimap.md F1 to F6): the central 75 % of the per-map image at
//! 512 x 512 ref px in a frame with the mode's legend, the things of
//! `scene` at their own transform (F3, `game::radar::maths::FullMap`), the
//! players' names under their icons, the team spawn markers, the grid with
//! its labels and up to 20 death marks.
//!
//! - F1: held (`+fulltacticalmap`) and no chat line open, no in-game menu,
//!   (held through round end and intermission); shown while dead too; the radar keeps drawing.
//! - F2: frame (90, 93) 844 x 584, legend panel (101, 103) 252 x 562, map
//!   panel (361, 103) 562 x 562, image (397, 139) at alpha 230 showing
//!   u, v in [0.125, 0.875]; the grid `DotGuide` at (392, 134) 523 x 523
//!   with column labels A to M (x = 406 + 40 i, y 110) and row labels 1 to
//!   13 (x 371, y = 144 + 40 i); the legend rows 400 x 32 at (101, 109 +
//!   37 i), the icon, its wedge over it and the text at (+69, +11).
//! - F4: what is shown: the radar's things less the heartbeat contacts
//!   (`scene`), the dead allies as death icons (a dead VIP as the VIP icon)
//!   and revealed dead enemies with their living icon, at the place they
//!   died (no wedge), the death
//!   marks, the names (centred under the icon: the box's (+16, +24), bold
//!   11, white, black outline).
//!
//! Headless state `CaFullMap`; `draw_full_map` draws it with the install's
//! art (flat coloured stand-ins without it, as the radar's).
//!
//! Known shortcuts: the frame, the legend panel and the map panel are
//! flat dark boxes (the layout's art for them isn't named by the spec);
//! the grid's labels are drawn in the shell's font, not a layout's; the
//! picture is dark where the picture is missing; the legend's descriptions
//! come from the string table by their id (the built-in list's from its own
//! English); an icon or name outside the map panel is drawn where it falls
//! (F3: nothing is clipped).

use std::collections::HashSet;

use bevy::prelude::*;

use super::{
    HudScreen, colour, layer,
    model::{PxRect, Screen},
    radar::{self, RadarKeys, spawn_box, spawn_quad},
    scene::{CaScene, DeathMarks, Kind},
};
use crate::{
    client::{chat::ChatInput, fonts::UiFonts},
    game::{
        radar::{
            CaRadarRound, CaWorldOffset,
            legend::{self, IconList},
            look::{self, Look},
            maths::{self, FullMap},
        },
    },
    CaUi,
        draw::{CaArt, CaFaces},
};

/// The full map's alpha (F2).
pub const MAP_ALPHA: u8 = 230;
/// The death mark's side, ref px.
pub const MARK_SIDE: f32 = 54.0;

/// An icon on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct MapMark {
    pub entity: Option<Entity>,
    pub look: Look,
    /// The icon's centre, ref px; its box (64 x 32, unscaled), screen px.
    pub centre: (i32, i32),
    pub rect: PxRect,
    pub wedge: Option<[Vec2; 4]>,
    /// The name and the centre of its text, screen px.
    pub name: Option<(String, Vec2)>,
}

/// The picture: its texture, the box it covers and the share of the
/// texture shown.
#[derive(Clone, Debug, PartialEq)]
pub struct MapPicture {
    pub texture: String,
    pub rect: PxRect,
    pub size: (i32, i32),
}

/// One legend row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The row's rect, ref px.
    pub at: (f32, f32),
    pub icon: String,
    pub view: Option<String>,
    pub text: String,
    /// The text's top-left, ref px.
    pub text_at: (f32, f32),
}

/// What the full map shows this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaFullMap {
    pub screen: Screen,
    pub visible: bool,
    pub picture: Option<MapPicture>,
    pub marks: Vec<MapMark>,
    /// The death marks' boxes, screen px.
    pub deaths: Vec<PxRect>,
    pub legend: Vec<Row>,
}

/// F3's transform for the round's image, None without data.
fn transform(round: &CaRadarRound, offset: Vec2) -> Option<FullMap> {
    round.image.corners.map(|c| FullMap::new(&c, (offset.x as f64, offset.y as f64)))
}

fn icon_box(centre: (i32, i32), s: Screen) -> PxRect {
    let (cx, cy) = (centre.0 as f32, centre.1 as f32);
    PxRect {
        min: s.at(Vec2::new(cx - 32.0, cy - 16.0)),
        max: s.at(Vec2::new(cx + 32.0, cy + 16.0)),
    }
}

/// F1: whether the map shows.
/// The map's own per-frame check has no round-end condition (corrections
/// 2026-10-22, 3): held, it stays through round end and intermission.
fn shown(keys: &RadarKeys, chat: Option<&ChatInput>, menu: bool) -> bool {
    keys.map && chat.is_none_or(|c| c.open.is_none()) && !menu
}

#[allow(clippy::too_many_arguments)]
pub fn update_full_map(
    scene: Res<CaScene>,
    keys: Res<RadarKeys>,
    chat: Option<Res<ChatInput>>,
    menu: Option<Res<crate::menu::InGameMenu>>,
    round: Res<CaRadarRound>,
    offset: Res<CaWorldOffset>,
    marks: Res<DeathMarks>,
    screen: Res<HudScreen>,
    ui: Option<Res<CaUi>>,
    mut map: ResMut<CaFullMap>,
) {
    let s = screen.0;
    let visible = shown(&keys, chat.as_deref(), menu.as_ref().is_some_and(|m| m.open));
    let mut next = CaFullMap {
        screen: s,
        visible,
        ..default()
    };
    if visible {
        let tf = transform(&round, offset.0);
        if let (Some(key), Some(_)) = (round.image.key(), tf) {
            next.picture = Some(MapPicture {
                texture: key,
                rect: PxRect {
                    min: s.at(Vec2::new(maths::MAP_AT.0 as f32, maths::MAP_AT.1 as f32)),
                    max: s.at(Vec2::new((maths::MAP_AT.0 + maths::MAP_SIDE) as f32, (maths::MAP_AT.1 + maths::MAP_SIDE) as f32)),
                },
                size: round.image.size,
            });
        }
        if let Some(tf) = tf {
            let (xr, yr) = (s.xr() as f64, s.yr() as f64);
            let place = |look: Look, at: (f64, f64), yaw: Option<f64>, entity: Option<Entity>, name: Option<&String>, down: i32| {
                let mut centre = tf.pixel(at);
                centre.1 += down;
                let rect = icon_box(centre, s);
                let wedge = yaw.filter(|_| look.view.is_some()).map(|y| maths::wedge(centre, y, 0.0, 1.0, xr, yr));
                let name = name.map(|n| {
                    // Centred under the icon: the box's (+16, +24).
                    (n.clone(), s.at(Vec2::new(centre.0 as f32 - 32.0 + 32.0, centre.1 as f32 - 16.0 + 24.0)))
                });
                MapMark {
                    entity,
                    look,
                    centre,
                    rect,
                    wedge,
                    name,
                }
            };
            for t in scene.things.iter().filter(|t| t.kind == Kind::Item) {
                if let Some(l) = t.map.clone() {
                    next.marks.push(place(l, t.at, None, t.entity, None, 0));
                }
            }
            for (l, at, down) in &scene.spawns {
                next.marks.push(place(l.clone(), *at, None, None, None, *down));
            }
            for t in scene.things.iter().filter(|t| t.kind == Kind::Character) {
                if let Some(l) = t.map.clone() {
                    let yaw = Some(t.yaw);
                    next.marks.push(place(l, t.at, yaw, t.entity, t.name.as_ref(), 0));
                }
            }
            if let Some(me) = &scene.me {
                let l = look::map_local(me.alive, &me.carry);
                next.marks.push(place(l, me.at, me.alive.then_some(me.yaw), Some(me.entity), me.name.as_ref(), 0));
            }
            // F4: the death marks, under the icons.
            for m in &marks.marks {
                let c = tf.pixel(m.0);
                let h = MARK_SIDE / 2.0;
                next.deaths.push(PxRect {
                    min: s.at(Vec2::new(c.0 as f32 - h, c.1 as f32 - h)),
                    max: s.at(Vec2::new(c.0 as f32 + h, c.1 as f32 + h)),
                });
            }
        }
        // F2: the legend.
        let builtin;
        let lists: &[IconList] = match ui.as_ref().and_then(|u| u.install.as_ref()).map(|i| &i.legend) {
            Some(l) if !l.is_empty() => l,
            _ => {
                builtin = legend::builtin();
                &builtin
            }
        };
        let game_type = legend::game_type(&scene.mode);
        if let Some(list) = legend::pick(lists, game_type) {
            for (i, item) in list.items.iter().enumerate() {
                let y = 109.0 + 37.0 * i as f32;
                let text = ui
                    .as_ref()
                    .and_then(|u| u.strings.values.get(&item.desc.trim_start_matches('$').to_lowercase()).cloned())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| item.text.clone());
                next.legend.push(Row {
                    at: (101.0, y),
                    icon: item.icon.clone(),
                    view: item.view.clone(),
                    text,
                    text_at: (101.0 + 69.0, y + 11.0),
                });
            }
        }
    }
    if *map != next {
        *map = next;
    }
}

/// A texture's flat tone for the panels.
const PANEL: [u8; 4] = [14, 18, 22, 235];

#[derive(Component)]
pub struct MapRoot;

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((MapRoot, layer(super::HUD_Z + 20), crate::client::hud::HudRoot(crate::game::GAME)));
}

fn spawn_text(p: &mut ChildSpawnerCommands, s: &str, centre: Vec2, size: f32, col: [u8; 4], fonts: Option<&UiFonts>, faces: Option<&CaFaces>, centred: bool, outline: bool) {
    let font = crate::draw::text_font("bold", size.round() as i32, fonts, faces);
    let w = (s.chars().count() as f32 * size * 0.7).max(8.0);
    let h = size * 1.3;
    let mut one = |off: Vec2, c: [u8; 4]| {
        let left = if centred { centre.x - w / 2.0 } else { centre.x } + off.x;
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(left),
                top: px(centre.y - h / 2.0 + off.y),
                width: px(w),
                height: px(h),
                justify_content: if centred { JustifyContent::Center } else { JustifyContent::Start },
                align_items: AlignItems::Center,
                overflow: Overflow::visible(),
                ..default()
            },
            children![(Text::new(s.to_string()), font.clone(), TextColor(colour(c)), TextLayout::no_wrap())],
        ));
    };
    if outline {
        for off in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
            one(off, [0, 0, 0, 255]);
        }
    }
    one(Vec2::ZERO, col);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_full_map(
    map: Res<CaFullMap>,
    mut art: Option<ResMut<CaArt>>,
    mut images: Option<ResMut<Assets<Image>>>,
    fonts: Option<Res<UiFonts>>,
    faces: Option<Res<CaFaces>>,
    root: Single<Entity, With<MapRoot>>,
    mut failed: Local<HashSet<String>>,
    mut last: Local<Option<(CaFullMap, bool)>>,
    mut commands: Commands,
) {
    if let (Some(m), Some(a), Some(i)) = (&map.picture, art.as_mut(), images.as_mut()) {
        radar::ensure_picture(&m.texture, a, i, &mut failed);
    }
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !super::rebuild(&mut last, ((*map).clone(), loaded), *root, &mut commands) {
        return;
    }
    if !map.visible {
        return;
    }
    let s = map.screen;
    let art = art.as_deref();
    let (fonts, faces) = (fonts.as_deref(), faces.as_deref());
    let boxr = |x: f32, y: f32, w: f32, h: f32| PxRect {
        min: s.at(Vec2::new(x, y)),
        max: s.at(Vec2::new(x + w, y + h)),
    };
    commands.entity(*root).with_children(|p| {
        // The frame, the legend panel and the map panel (F2).
        for (x, y, w, h) in [(90.0, 93.0, 844.0, 584.0), (101.0, 103.0, 252.0, 562.0), (361.0, 103.0, 562.0, 562.0)] {
            let r = boxr(x, y, w, h);
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(r.min.x),
                    top: px(r.min.y),
                    width: px(r.size().x),
                    height: px(r.size().y),
                    ..default()
                },
                BackgroundColor(colour(PANEL)),
            ));
        }
        // The picture: the central 75 % of the texture (F2).
        if let Some(pic) = &map.picture
            && let Some((image, size)) = art.and_then(|a| a.get(&pic.texture))
        {
            let (w, h) = (size.x as f32, size.y as f32);
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(pic.rect.min.x),
                    top: px(pic.rect.min.y),
                    width: px(pic.rect.size().x),
                    height: px(pic.rect.size().y),
                    ..default()
                },
                ImageNode {
                    image: image.clone(),
                    color: colour([255, 255, 255, MAP_ALPHA]),
                    rect: Some(Rect::new(
                        w * maths::MAP_CROP as f32,
                        h * maths::MAP_CROP as f32,
                        w * (1.0 - maths::MAP_CROP as f32),
                        h * (1.0 - maths::MAP_CROP as f32),
                    )),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        // The grid and its labels (decoration).
        spawn_box(p, Vec2::ZERO, look::GRID, boxr(392.0, 134.0, 523.0, 523.0), [255; 4], art);
        let grey = [179, 179, 179, 255];
        for i in 0..13 {
            let c = s.at(Vec2::new(406.0 + 40.0 * i as f32 + 10.0, 110.0 + 10.0));
            spawn_text(p, &((b'A' + i as u8) as char).to_string(), c, 20.0 * s.yr(), grey, fonts, faces, true, false);
            let c = s.at(Vec2::new(371.0 + 10.0, 144.0 + 40.0 * i as f32 + 10.0));
            spawn_text(p, &(i + 1).to_string(), c, 20.0 * s.yr(), grey, fonts, faces, true, false);
        }
        // The death marks, then the icons.
        for r in &map.deaths {
            spawn_box(p, Vec2::ZERO, &look::icon(look::DEATH_MARK), *r, [255; 4], art);
        }
        for m in &map.marks {
            spawn_box(p, Vec2::ZERO, &m.look.icon, m.rect, [255; 4], art);
            if let Some(b) = &m.look.badge {
                spawn_box(p, Vec2::ZERO, b, m.rect, [255; 4], art);
            }
            if let (Some(w), Some(view)) = (m.wedge, &m.look.view) {
                spawn_quad(p, Vec2::ZERO, view, radar::upright(w, s), [255; 4], art);
            }
            if let Some((name, at)) = &m.name {
                spawn_text(p, name, *at, 11.0 * s.yr(), [255; 4], fonts, faces, true, true);
            }
        }
        // The legend.
        for row in &map.legend {
            let icon = boxr(row.at.0, row.at.1, 64.0, 32.0);
            spawn_box(p, Vec2::ZERO, &row.icon, icon, [255; 4], art);
            if let Some(v) = &row.view {
                spawn_box(p, Vec2::ZERO, v, icon, [255; 4], art);
            }
            let at = s.at(Vec2::new(row.text_at.0, row.text_at.1 + 6.0));
            spawn_text(p, &row.text, at, 13.0 * s.yr(), grey, fonts, faces, false, true);
        }
    });
}
