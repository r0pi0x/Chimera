//! The radar, the HUD's corner map (specs/combat_arms/radar_minimap.md R2
//! to R8, R10 to R12): a clipped, moving window of the per-map image, the
//! things of `scene` as icons with facing wedges, edge arrows for the
//! objectives out of range, the player's own guide, the zoom bar and the
//! frame. All numbers are `game::radar::maths`'s; this module keeps the
//! zoom, decides when the panel is shown and turns the scene into pixels
//! (`CaRadar`, headless) and into UI nodes (`draw_radar`).
//!
//! - R2: shown while the local player lives (the death camera hides it
//!   while dead); hidden, it saves the zoom to the profile
//!   (`CaRadarProfile::scale`); entering a world (`MapLoads`) shows it,
//!   restores the zoom (clamped) and latches the rotate option (R10: a
//!   change mid-map waits for the next).
//! - Not shown at all with `hud_radar 0` (`client::hud::Radars`: the
//!   party apps' default).
//! - R3: `+incradarscale` / `+decradarscale` (`=` and `-`) change the zoom
//!   by 0.01 per rendered frame while held and no chat line is open.
//! - R4 to R8: in-range things at their pixel, wedges for characters, the
//!   guide, arrows for things out of range.
//!
//! Drawing: the install's art (the icons under `UI_HUD_Map\icon\`, the map
//! image under `UI_HUD_Map\mapImage\`, the radar art) through the shell's
//! UI nodes, clipped to the window; without it flat coloured boxes by role
//! and a dark window. Bevy's clip leaves turned nodes whole, so the turned
//! quads are `client::clip_quad` materials cut to the window by the
//! shader: the wedges, arrows and guide are turned in square space
//! (`upright`: the angle follows the view's, no shear on a screen that
//! isn't 4:3), the map picture is the exact stretched quad so it keeps to
//! the icons' places. The picture's texture window outside the image
//! (near the map's edge) is clamp-to-edge (Corrections 2026-10-28, 2:
//! `clip_quad.wesl`), not empty.
//!
//! Known shortcuts: the radar hides while the local player is dead (the
//! death camera, item 26, isn't there); letterbox, video and AI freeze
//! never hide it; the co-op HUD mode's second look (R12) isn't there; icons that
//! cross the window's edge are clipped, the zoom bar and frame drawn over
//! them.

use std::collections::HashSet;

use bevy::{prelude::*, ui::UiTransform};

use super::{
    HudScreen, colour, layer,
    model::{PxRect, Screen},
    scene::{CaScene, Kind, Me, Thing, frame},
};
use crate::{
    client::{
        chat::ChatInput,
        clip_quad::{ClipQuadMaterial, quad_node},
    },
    core::MapLoads,
    game::radar::{
        CaRadarProfile, CaRadarRound, CaWorldOffset,
        look::{self, Look},
        maths::{self, Corners, Window},
    },
    ui::{
        draw::CaArt,
        install::texture_key,
    },
};

/// The radar map's alpha (R-constants).
pub const MAP_ALPHA: u8 = 190;
/// The frame's place (layout `bg`), the zoom bar's track and knob, ref px.
/// The control is (782, -30) 227 x 226, which shows the top-left 227 x 226
/// texels of the 256 x 256 `bg.dtx` at one texel per ref px (Corrections
/// 2026-10-28, 1); the rest of the texture is transparent, so the whole
/// image drawn at (782, -30) 256 x 256 is the same picture. Its outline's
/// columns are texels 35 and 221 and its rows 34 and 220 (`OUTLINE`).
pub const FRAME_AT: (f32, f32, f32, f32) = (782.0, -30.0, 256.0, 256.0);
/// The outline in `bg.dtx`: columns, rows (texels, both ends included).
pub const FRAME_OUTLINE: ((f32, f32), (f32, f32)) = ((35.0, 221.0), (34.0, 220.0));
pub const TRACK_AT: (f32, f32, f32, f32) = (1004.0, 4.0, 16.0, 192.0);
pub const KNOB_SIZE: (f32, f32) = (16.0, 10.0);

/// The held keys (R3, F1).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct RadarKeys {
    /// `IncRadarScale`, `DecRadarScale`.
    pub expand: bool,
    pub decrease: bool,
    /// `FullTacticalMap`.
    pub map: bool,
}

/// An icon on the radar: where it is and how it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub entity: Option<Entity>,
    pub look: Look,
    /// The icon's centre, ref px, and its box in screen px (scaled by the
    /// zoom about the centre).
    pub centre: (i32, i32),
    pub rect: PxRect,
    /// The facing wedge's corners, screen px.
    pub wedge: Option<[Vec2; 4]>,
}

/// An edge arrow (R8).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeArrow {
    pub entity: Option<Entity>,
    pub look: Look,
    /// The arrow's point, ref px; its box's top-left is the point less
    /// (32, 16).
    pub point: (i32, i32),
    pub beta: f64,
    /// The background arrow's corners, screen px, and the icon's box.
    pub corners: [Vec2; 4],
    pub icon: PxRect,
}

impl EdgeArrow {
    /// The image drawn over the turned arrow: a flag carrier's badge in
    /// place of its character icon, else the icon (corrections
    /// 2026-10-22, 2).
    pub fn icon_image(&self) -> &str {
        self.look.badge.as_deref().unwrap_or(&self.look.icon)
    }
}

/// The map image under the window (R5).
#[derive(Clone, Debug, PartialEq)]
pub struct MapQuad {
    /// The picture's texture key.
    pub texture: String,
    /// The quad's corners, screen px.
    pub corners: [Vec2; 4],
    pub window: Window,
    pub size: (i32, i32),
    pub corners_world: Corners,
}

/// What the radar shows this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaRadar {
    pub screen: Screen,
    pub visible: bool,
    pub zoom: f32,
    /// The rotate option as latched when the world was entered.
    pub rotate: bool,
    /// The zoom bar knob's y, ref px.
    pub knob: i32,
    /// The window's rect, screen px.
    pub window: Option<PxRect>,
    pub map: Option<MapQuad>,
    pub marks: Vec<Mark>,
    pub arrows: Vec<EdgeArrow>,
    /// The player's guide, screen px.
    pub guide: Option<[Vec2; 4]>,
}

/// What the radar remembers between frames.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RadarMemory {
    pub loads: Option<u32>,
    pub was_visible: bool,
    pub zoom: f32,
    pub rotate: bool,
}

fn scaled(r: PxRect, s: Screen) -> PxRect {
    PxRect {
        min: s.at(r.min),
        max: s.at(r.max),
    }
}

/// The keys' commands (R3, F1).
pub fn register_keys(app: &mut App) {
    use crate::console::ConsoleAppExt;
    for (name, which) in [("incradarscale", 0), ("decradarscale", 1), ("fulltacticalmap", 2)] {
        for (sign, on) in [("+", true), ("-", false)] {
            app.console_command(&format!("{sign}{name}"), "A radar key (for binds).", move |w, _| {
                if let Some(mut k) = w.get_resource_mut::<RadarKeys>() {
                    match which {
                        0 => k.expand = on,
                        1 => k.decrease = on,
                        _ => k.map = on,
                    }
                }
                Ok(None)
            });
        }
    }
}

/// R2, R3: the radar's visibility, zoom and what it shows.
#[allow(clippy::too_many_arguments)]
pub fn update_radar(
    scene: Res<CaScene>,
    keys: Res<RadarKeys>,
    chat: Option<Res<ChatInput>>,
    switch: Option<Res<crate::game::options::CaHudSwitch>>,
    loads: Option<Res<MapLoads>>,
    offset: Res<CaWorldOffset>,
    round: Res<CaRadarRound>,
    screen: Res<HudScreen>,
    radars: Option<Res<crate::client::hud::Radars>>,
    mut profile: ResMut<CaRadarProfile>,
    mut memory: ResMut<RadarMemory>,
    mut radar: ResMut<CaRadar>,
) {
    let s = screen.0;
    let l = loads.map_or(0, |l| l.0);
    // Entering a world (R2): visible, the zoom restored, the rotate
    // option read.
    if memory.loads != Some(l) {
        memory.loads = Some(l);
        memory.zoom = maths::zoom_restore(profile.scale);
        memory.rotate = profile.rotate != 0;
        memory.was_visible = false;
    }
    let me = scene.me.as_ref();
    let visible = me.is_some_and(|m| m.alive);
    // The HUD on/off key hides the panels, the radar among them (its zoom
    // is kept and doesn't change meanwhile: the radar isn't shown).
    // `hud_radar 0` (`client::hud::Radars`, the party apps' default) too.
    let visible_now = visible && switch.is_none_or(|w| w.panels()) && radars.is_none_or(|r| r.shown());
    if !visible && memory.was_visible {
        // Hiding saves the zoom to the profile.
        profile.scale = memory.zoom;
    }
    memory.was_visible = visible;
    if visible_now && chat.is_none_or(|c| c.open.is_none()) {
        memory.zoom = maths::zoom_step(memory.zoom, keys.expand, keys.decrease);
    }
    let mut next = CaRadar {
        screen: s,
        visible: visible_now,
        zoom: memory.zoom,
        rotate: memory.rotate,
        knob: maths::knob_y(memory.zoom),
        ..default()
    };
    if let (true, Some(me)) = (visible_now, me) {
        fill(&mut next, me, &scene, offset.0, &round);
    }
    if *radar != next {
        *radar = next;
    }
}

fn fill(out: &mut CaRadar, me: &Me, scene: &CaScene, offset: Vec2, round: &CaRadarRound) {
    let s = out.screen;
    let (xr, yr) = (s.xr() as f64, s.yr() as f64);
    let f = frame(me, out.zoom, out.rotate);
    out.window = Some(scaled(
        PxRect {
            min: Vec2::new(maths::WINDOW_AT.0 as f32, maths::WINDOW_AT.1 as f32),
            max: Vec2::new((maths::WINDOW_AT.0 + maths::WINDOW_SIDE) as f32, (maths::WINDOW_AT.1 + maths::WINDOW_SIDE) as f32),
        },
        s,
    ));
    // R5: the picture under the window.
    if let (Some(c), Some(key)) = (round.image.corners, round.image.key()) {
        out.map = Some(MapQuad {
            texture: key,
            corners: maths::map_quad(&f, xr, yr),
            window: maths::window(&f, (offset.x as f64, offset.y as f64), &c),
            size: round.image.size,
            corners_world: c,
        });
    }
    // R7: the guide.
    out.guide = Some(maths::guide(&f, xr, yr));
    // R6: the things, in draw order (items, AI, players).
    for t in &scene.things {
        let Some(look) = t.radar.clone() else { continue };
        match f.pixel(t.at) {
            Some(centre) => out.marks.push(mark(t, look, centre, &f, s)),
            None if look.arrow => {
                // R8: the direction from the player, turned in the
                // rotating mode, with the height difference.
                let d = f.offset(t.at);
                let a = maths::arrow(d.x, t.y - me.y, d.y);
                let icon = PxRect::new(a.point.0 as f32 - 32.0, a.point.0 as f32 + 32.0, a.point.1 as f32 - 16.0, a.point.1 as f32 + 16.0);
                out.arrows.push(EdgeArrow {
                    entity: t.entity,
                    look,
                    point: a.point,
                    beta: a.beta,
                    corners: maths::arrow_corners(&a, xr, yr),
                    icon: scaled(icon, s),
                });
            }
            None => {}
        }
    }
}

fn mark(t: &Thing, look: Look, centre: (i32, i32), f: &maths::Frame, s: Screen) -> Mark {
    let z = f.zoom as f32;
    let (cx, cy) = (centre.0 as f32, centre.1 as f32);
    let rect = scaled(PxRect::new(cx - 32.0 * z, cx + 32.0 * z, cy - 16.0 * z, cy + 16.0 * z), s);
    let wedge = (t.kind == Kind::Character && look.view.is_some())
        .then(|| maths::wedge(centre, t.yaw, f.rot(), f.zoom, s.xr() as f64, s.yr() as f64));
    Mark {
        entity: t.entity,
        look,
        centre,
        rect,
        wedge,
    }
}

// ---------------------------------------------------------------------------
// Drawing

/// The whole image over a quad.
const FULL: Vec4 = Vec4::new(0.0, 0.0, 1.0, 1.0);

#[derive(Component)]
pub struct RadarRoot;

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((RadarRoot, layer(super::HUD_Z - 3), crate::client::hud::HudRoot(crate::game::GAME)));
}

/// The icon files and radar art the radar draws, as texture keys (the
/// install's UI art loads them with the layouts').
pub fn art_names() -> Vec<String> {
    let mut v: Vec<String> = [
        "character_ally.dtx",
        "character_ally_view.dtx",
        "character_ally_money.DTX",
        "character_ally_bomb.dtx",
        "character_enemy.dtx",
        "character_enemy_view.dtx",
        "character_enemy_money.dtx",
        "character_enemy_bomb.dtx",
        "character_spy_pre.DTX",
        "character_spy_view.dtx",
        "character_VIP.dtx",
        "character_VIP_View.dtx",
        "character_flag_A.dtx",
        "character_flag_B.dtx",
        "character_player.dtx",
        "character_player_view.dtx",
        "character_player_bomb.dtx",
        "character_player_money.dtx",
        "character_NPC.dtx",
        "character_NPC03.dtx",
        "character_NPC_view.dtx",
        "area_bomb.dtx",
        "area_bomb_A.dtx",
        "area_bomb_B.dtx",
        "area_goal.dtx",
        "area_flag_A.dtx",
        "area_flag_B.dtx",
        "area_flagPoint_A.dtx",
        "area_flagPoint_B.dtx",
        "area_respawn_A.dtx",
        "area_respawn_B.dtx",
        "area_respawn_DF.dtx",
        "area_respawn_OF.dtx",
        "area_respawn.dtx",
        "area_death.dtx",
        "area_death_ally.DTX",
        "area_death_player.DTX",
        "item_bomb.DTX",
        "item_intel.dtx",
        "item_money.DTX",
    ]
    .iter()
    .map(|f| look::icon(f))
    .collect();
    v.extend(
        [look::GUIDE, look::ARROW, look::FRAME, look::MASK, look::ZOOM_TRACK, look::ZOOM_KNOB, look::GRID]
            .iter()
            .map(|s| s.to_string()),
    );
    v
}

/// A flat colour standing for a texture without the install: by role.
pub fn flat(texture: &str) -> [u8; 4] {
    let n = texture.to_lowercase();
    let has = |s: &str| n.contains(s);
    if has("death") {
        [200, 200, 200, 255]
    } else if has("enemy") {
        [220, 60, 50, 255]
    } else if has("spy") {
        [240, 150, 40, 255]
    } else if has("ally") || has("player") {
        [70, 140, 230, 255]
    } else if has("npc") {
        [180, 100, 200, 255]
    } else if has("flag") || has("bomb") || has("goal") || has("intel") || has("money") {
        [240, 210, 60, 255]
    } else if has("respawn") {
        [120, 200, 120, 255]
    } else if has("locationindicator") {
        [240, 240, 240, 255]
    } else {
        [150, 150, 150, 255]
    }
}

/// An image or its flat stand-in over the rotated quad `corners` (screen
/// px, relative to `origin`), tinted by `tint`.
pub fn spawn_quad(p: &mut ChildSpawnerCommands, origin: Vec2, texture: &str, corners: [Vec2; 4], tint: [u8; 4], art: Option<&CaArt>) {
    let [p0, p1, _, p3] = corners;
    let (u, v) = (p1 - p0, p3 - p0);
    let (w, h) = (u.length(), v.length());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let centre = (corners[0] + corners[2]) / 2.0 - origin;
    let angle = u.y.atan2(u.x);
    let node = Node {
        position_type: PositionType::Absolute,
        left: px(centre.x - w / 2.0),
        top: px(centre.y - h / 2.0),
        width: px(w),
        height: px(h),
        ..default()
    };
    let tf = UiTransform::from_rotation(Rot2::radians(angle));
    match art.and_then(|a| a.get(texture)) {
        Some((image, _)) => {
            p.spawn((
                node,
                tf,
                ImageNode {
                    image: image.clone(),
                    color: colour(tint),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        None => {
            let c = flat(texture);
            let m = |a: u8, b: u8| ((a as u32 * b as u32) / 255) as u8;
            p.spawn((node, tf, BackgroundColor(colour([m(c[0], tint[0]), m(c[1], tint[1]), m(c[2], tint[2]), m(c[3], tint[3])]))));
        }
    }
}

/// An axis-aligned box of the texture over `r` (screen px).
pub fn spawn_box(p: &mut ChildSpawnerCommands, origin: Vec2, texture: &str, r: PxRect, tint: [u8; 4], art: Option<&CaArt>) {
    let corners = [r.min, Vec2::new(r.max.x, r.min.y), r.max, Vec2::new(r.min.x, r.max.y)];
    spawn_quad(p, origin, texture, corners, tint, art);
}

/// A spec quad (corners in screen px, turned in reference px and then
/// stretched by (x_r, y_r), which shears it on a screen that isn't 4:3)
/// as a quad turned in square space: the same centre, angle and sides in
/// reference px, drawn at the one scale `min(x_r, y_r)`. A wedge or guide
/// then turns at the view's speed and keeps its shape.
pub fn upright(corners: [Vec2; 4], s: Screen) -> [Vec2; 4] {
    let (xr, yr) = (s.xr(), s.yr());
    let k = xr.min(yr);
    let back = |v: Vec2| Vec2::new(v.x / xr, v.y / yr) * k;
    let centre = (corners[0] + corners[2]) / 2.0;
    let (u, v) = (back(corners[1] - corners[0]), back(corners[3] - corners[0]));
    [centre - (u + v) / 2.0, centre + (u - v) / 2.0, centre + (u + v) / 2.0, centre + (v - u) / 2.0]
}

/// A turned quad of the texture (or its flat stand-in) with its `uv` part,
/// cut to the window by the shader (Bevy's clip leaves turned nodes
/// whole). `corners` are screen px, `origin` the clip node's top-left,
/// `clip_size` its size.
#[allow(clippy::too_many_arguments)]
pub fn spawn_turned(
    p: &mut ChildSpawnerCommands,
    materials: &mut Assets<ClipQuadMaterial>,
    origin: Vec2,
    clip_size: Vec2,
    texture: &str,
    corners: [Vec2; 4],
    tint: [u8; 4],
    uv: Vec4,
    art: Option<&CaArt>,
) {
    let (image, color) = match art.and_then(|a| a.get(texture)) {
        Some((image, _)) => (Some(image.clone()), colour(tint)),
        None => {
            let c = flat(texture);
            let m = |a: u8, b: u8| ((a as u32 * b as u32) / 255) as u8;
            (None, colour([m(c[0], tint[0]), m(c[1], tint[1]), m(c[2], tint[2]), m(c[3], tint[3])]))
        }
    };
    let rel = corners.map(|c| c - origin);
    if let Some(q) = quad_node(rel, (Vec2::ZERO, clip_size), color, image, uv) {
        p.spawn((q.node, MaterialNode(materials.add(q.material))));
    }
}

/// Pictures decoded on first use (the map images are large): the keys
/// that failed aren't tried again.
pub fn ensure_picture(key: &str, art: &mut CaArt, images: &mut Assets<Image>, failed: &mut HashSet<String>) -> bool {
    if art.get(key).is_some() {
        return true;
    }
    if failed.contains(key) {
        return false;
    }
    match crate::game::INFO.mount().and_then(|m| crate::ui::install::decode(&m, key)) {
        Some(a) => {
            let handle = images.add(crate::ui::draw::rgba_image(&a));
            art.0.insert(texture_key(key), (handle, a.size));
            true
        }
        None => {
            failed.insert(key.to_string());
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_radar(
    radar: Res<CaRadar>,
    mut art: Option<ResMut<CaArt>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut materials: Option<ResMut<Assets<ClipQuadMaterial>>>,
    root: Single<Entity, With<RadarRoot>>,
    mut failed: Local<HashSet<String>>,
    mut last: Local<Option<(CaRadar, bool)>>,
    mut commands: Commands,
) {
    // The map picture, decoded when first wanted.
    if let (Some(m), Some(a), Some(i)) = (&radar.map, art.as_mut(), images.as_mut()) {
        ensure_picture(&m.texture, a, i, &mut failed);
    }
    // Icons the data names (the map markers' trigger resource pictures),
    // decoded when first wanted too.
    if let (Some(a), Some(i)) = (art.as_mut(), images.as_mut()) {
        for m in &radar.marks {
            ensure_picture(&m.look.icon, a, i, &mut failed);
        }
    }
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !super::rebuild(&mut last, ((*radar).clone(), loaded), *root, &mut commands) {
        return;
    }
    if !radar.visible {
        return;
    }
    let (Some(window), s) = (radar.window, radar.screen) else { return };
    let art = art.as_deref();
    let origin = window.min;
    let clip = window.size();
    commands.entity(*root).with_children(|p| {
        // The window: a clip over the picture, the things and the guide.
        p.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(window.min.x),
            top: px(window.min.y),
            width: px(window.size().x),
            height: px(window.size().y),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|c| {
            let alpha = [255, 255, 255, MAP_ALPHA];
            match radar.map.as_ref().filter(|m| art.is_some_and(|a| a.get(&m.texture).is_some())) {
                Some(m) => {
                    if let Some(mats) = materials.as_mut() {
                        let w = m.window;
                        let uv = Vec4::new(w.u0 as f32, w.v0 as f32, w.uw as f32, w.vh as f32);
                        // The picture keeps the stretch of the icons'
                        // places, so it is the exact parallelogram.
                        spawn_turned(c, mats, origin, clip, &m.texture, m.corners, alpha, uv, art);
                    }
                }
                None => {
                    c.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            top: px(0.0),
                            width: percent(100.0),
                            height: percent(100.0),
                            ..default()
                        },
                        BackgroundColor(colour([16, 20, 24, MAP_ALPHA])),
                    ));
                }
            }
            for m in &radar.marks {
                spawn_box(c, origin, &m.look.icon, m.rect, [255; 4], art);
                if let Some(b) = m.look.badge.as_ref().filter(|b| !b.is_empty()) {
                    spawn_box(c, origin, b, m.rect, [255; 4], art);
                }
                if let (Some(w), Some(view), Some(mats)) = (m.wedge, &m.look.view, materials.as_mut()) {
                    spawn_turned(c, mats, origin, clip, view, upright(w, s), [255; 4], FULL, art);
                }
            }
            for a in &radar.arrows {
                if let Some(mats) = materials.as_mut() {
                    spawn_turned(c, mats, origin, clip, look::ARROW, upright(a.corners, s), [255; 4], FULL, art);
                }
                // A flag carrier's arrow shows only the flag badge, in place of
                // the character icon (corrections 2026-10-22, 2).
                if !a.icon_image().is_empty() {
                    spawn_box(c, origin, a.icon_image(), a.icon, [255; 4], art);
                }
            }
            if let (Some(g), Some(mats)) = (radar.guide, materials.as_mut()) {
                spawn_turned(c, mats, origin, clip, look::GUIDE, upright(g, s), [255; 4], FULL, art);
            }
        });
        // The frame and the zoom bar, over everything and unclipped.
        let rect = |x: f32, y: f32, w: f32, h: f32| scaled(PxRect::new(x, x + w, y, y + h), s);
        let (fx, fy, fw, fh) = FRAME_AT;
        // Without the install the flat stand-in would hide the window.
        if art.is_some_and(|a| a.get(look::FRAME).is_some()) {
            spawn_box(p, Vec2::ZERO, look::FRAME, rect(fx, fy, fw, fh), [255; 4], art);
        }
        let (tx, ty, tw, th) = TRACK_AT;
        // The track is 16 x 256 in a 16 x 192 rect: its top 192 rows at one
        // texel per ref px, not squeezed (Corrections 2026-10-28, 1).
        match art.and_then(|a| a.get(look::ZOOM_TRACK)) {
            Some((image, size)) => {
                let r = rect(tx, ty, tw, th);
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(r.min.x),
                        top: px(r.min.y),
                        width: px(r.size().x),
                        height: px(r.size().y),
                        ..default()
                    },
                    ImageNode {
                        image: image.clone(),
                        rect: Some(bevy::math::Rect::new(0.0, 0.0, tw.min(size.x as f32), th.min(size.y as f32))),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                ));
            }
            None => spawn_box(p, Vec2::ZERO, look::ZOOM_TRACK, rect(tx, ty, tw, th), [255; 4], art),
        }
        spawn_box(
            p,
            Vec2::ZERO,
            look::ZOOM_KNOB,
            rect(maths::KNOB_X as f32, radar.knob as f32, KNOB_SIZE.0, KNOB_SIZE.1),
            [255; 4],
            art,
        );
    });
}

pub struct CaRadarStatePlugin;

impl Plugin for CaRadarStatePlugin {
    fn build(&self, app: &mut App) {
        register_keys(app);
        app.init_resource::<CaScene>()
            .init_resource::<super::scene::DeathMarks>()
            .init_resource::<super::scene::DeadSpots>()
            .init_resource::<RadarKeys>()
            .init_resource::<RadarMemory>()
            .init_resource::<CaRadar>()
            .init_resource::<super::full_map::CaFullMap>()
            .init_resource::<CaRadarProfile>()
            .init_resource::<CaWorldOffset>()
            .init_resource::<CaRadarRound>()
            .add_message::<crate::core::Died>()
            .add_systems(
                Update,
                (super::scene::track_deaths, super::scene::update_scene, update_radar, super::full_map::update_full_map)
                    .chain()
                    .after(super::HudUpdate)
                    .in_set(RadarUpdate),
            );
    }
}

/// The radar's and the map's update, after the HUD's.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RadarUpdate;
