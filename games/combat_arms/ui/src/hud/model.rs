//! Combat Arms' HUD behaviour as plain functions and state, with no ECS
//! or drawing (specs/combat_arms/hud_crosshair.md; section names below
//! are the spec's): what is shown when, where, in which colour, with
//! which text. `ui::hud` feeds it from the world each frame and draws the
//! result; the tests check it directly against the spec's test cases.
//!
//! Units: reference pixels on the 1024x768 canvas ("ref"), screen pixels
//! (px, `Screen`), milliseconds for the HUD's wall-clock timers, seconds
//! for the crosshair kick, radians. Colours are 0-255 RGBA.

use std::collections::VecDeque;
use std::f32::consts::{FRAC_1_SQRT_2, PI, TAU};

use bevy::math::{Vec2, Vec3};

/// An RGBA colour, 0-255 each.
pub type Rgba = [u8; 4];

// ---------------------------------------------------------------------------
// Constants ("Constants")

pub const REF_W: f32 = 1024.0;
pub const REF_H: f32 = 768.0;
/// Crosshair arm length L0, ref px.
pub const ARM_LENGTH: f32 = 7.0;
/// The firing kick: outward jump and extra length (ref px) and its
/// duration (s).
pub const KICK_OFFSET: f32 = 4.0;
pub const KICK_LENGTH: f32 = 6.0;
pub const KICK_TIME: f32 = 0.1;
pub const DEFAULT_CROSSHAIR: Rgba = [0, 255, 0, 255];
pub const HIT_CROSSHAIR: Rgba = [208, 0, 0, 255];
/// The hit indicator's time after the latest hit, ms.
pub const HIT_TIME_MS: f64 = 500.0;
/// The damage overlay's life (ms) and fade per HUD frame.
pub const DAMAGE_LIFE_MS: f64 = 1000.0;
pub const DAMAGE_ALPHA_STEP: u8 = 2;
/// The direction indicator's square side, ref px, and its texture's size.
pub const INDICATOR_SIDE: f32 = 338.0;
pub const INDICATOR_TEXTURE: Vec2 = Vec2::new(512.0, 256.0);
/// Used literally (not 1/6).
pub const SECTOR_FACTOR: f32 = 0.1666;
/// The scope range readout: position (ref, right-aligned), divisor and
/// colours.
pub const RANGE_AT: Vec2 = Vec2::new(490.0, 400.0);
pub const RANGE_DIVISOR: f32 = 40.0;
pub const RANGE_IN: Rgba = [255, 198, 0, 255];
pub const RANGE_OUT: Rgba = [255, 0, 0, 255];
/// The range readout's font: the font list's `SysFont13` asked for at
/// 20 px (Corrections 15; ours under this name, `ui::CaUi`).
pub const RANGE_FONT: &str = "SysFont13 at 20";
/// That font list entry's number (`SysFont13`).
pub const RANGE_SYSFONT: usize = 13;
/// Low health: at or below this percentage; its colour; the heartbeat's
/// period (more than this many OS ms since the last restart, D2).
pub const LOW_HEALTH_PERCENT: i32 = 30;
pub const LOW_HEALTH: Rgba = [255, 0, 0, 255];
pub const HEARTBEAT_MS: f64 = 1000.0;
/// A player's no-damage window as a shooter's client sees it (A10,
/// Corrections 1): this many OS ms from its (re)spawn, or until its first
/// shot.
pub const SPAWN_WINDOW_MS: f64 = 3500.0;
/// An action message's life after its last request, OS ms (D9).
pub const ACTION_MESSAGE_MS: f64 = 100.0;
/// The kill feed.
pub const FEED_LINES: usize = 5;
pub const FEED_LIFE_MS: f64 = 30000.0;
pub const FEED_LINE_HEIGHT: f32 = 20.0;
pub const FEED_TOP: f32 = 45.0;
pub const FEED_LEFT: f32 = 5.0;
pub const FEED_ICON_WIDTH: f32 = 64.0;
pub const FEED_BADGE_WIDTH: f32 = 32.0;
pub const FEED_MATE: [u8; 3] = [124, 190, 221];
pub const FEED_OTHER: [u8; 3] = [218, 115, 115];
pub const FEED_TEXT_ALPHA: u8 = 150;
/// Name tags.
pub const TAG_FLASH_S: f64 = 0.2;
pub const TAG_MATE: Rgba = [161, 197, 233, 200];
pub const TAG_ENEMY: Rgba = [215, 17, 18, 200];
pub const TAG_FLASH: Rgba = [250, 80, 0, 200];
pub const TAG_DEAD: Rgba = [80, 80, 80, 200];
pub const TAG_SEEN: u8 = 255;
pub const TAG_OCCLUDED: u8 = 125;
/// The aim ray's reach, world units.
pub const AIM_RANGE: f32 = 100000.0;

// ---------------------------------------------------------------------------
// The screen ("Units and conventions", D1)

/// The screen the HUD is drawn on, px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub w: f32,
    pub h: f32,
}

impl Default for Screen {
    fn default() -> Self {
        Self { w: REF_W, h: REF_H }
    }
}

impl Screen {
    pub fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
    pub fn xr(self) -> f32 {
        self.w / REF_W
    }
    pub fn yr(self) -> f32 {
        self.h / REF_H
    }
    /// The crosshair's centre (A1).
    pub fn centre(self) -> Vec2 {
        Vec2::new((self.w / 2.0).floor(), (self.h / 2.0).floor())
    }
    /// A reference point on the screen (x by x_r, y by y_r).
    pub fn at(self, p: Vec2) -> Vec2 {
        Vec2::new(p.x * self.xr(), p.y * self.yr())
    }
    /// A reference rect (position, size) on the screen.
    pub fn rect(self, at: Vec2, size: Vec2) -> PxRect {
        let min = self.at(at);
        PxRect {
            min,
            max: min + self.at(size),
        }
    }
}

/// A rect [min.x, max.x) x [min.y, max.y), px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl PxRect {
    pub fn new(x0: f32, x1: f32, y0: f32, y1: f32) -> Self {
        Self {
            min: Vec2::new(x0, y0),
            max: Vec2::new(x1, y1),
        }
    }
    pub fn size(self) -> Vec2 {
        self.max - self.min
    }
}

// ---------------------------------------------------------------------------
// The crosshair (A1 to A10)

/// A weapon's perturb fields the crosshair reads (A3): integers, perturb
/// units (one is one ref px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Perturb {
    pub min: i32,
    pub max: i32,
    /// `ZoomedMinPerturb`, `ZoomedMaxPerturb` (0: none).
    pub zoomed_min: i32,
    pub zoomed_max: i32,
    /// `TripleShotMinPerturb`, `TripleShotMaxPerturb` (0: none).
    pub burst_min: i32,
    pub burst_max: i32,
    /// `DuckPerturb`, `MoveDuckPerturb` (both above 0 to apply).
    pub duck: i32,
    pub move_duck: i32,
}

/// What changes the limits besides the weapon (A3); the defaults change
/// nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct PerturbMods {
    /// The weapon-upgrade modifier.
    pub upgrade: f32,
    /// The set-item rate on the maximum.
    pub set_rate: f32,
    /// Each fitted attachment's minimum and maximum perturb, slot order
    /// (the zoomed limits add their sums).
    pub attachments: Vec<(i32, i32)>,
    /// A fitted scope's minimum-perturb percentage (a fraction).
    pub scope_min: Option<f32>,
}

impl Default for PerturbMods {
    fn default() -> Self {
        Self {
            upgrade: 0.0,
            set_rate: 1.0,
            attachments: Vec::new(),
            scope_min: None,
        }
    }
}

impl PerturbMods {
    /// The attachments' sums (S_min, S_max), each S <- 2S + v (quirk 13).
    pub fn sums(&self) -> (i32, i32) {
        self.attachments
            .iter()
            .fold((0, 0), |(a, b), &(min, max)| (2 * a + min, 2 * b + max))
    }
}

/// The player's state as A3 reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stance {
    /// Zoomed in with no scope overlay.
    pub zoomed: bool,
    pub burst: bool,
    /// The crouch input held.
    pub crouch: bool,
    /// Moving or running.
    pub moving: bool,
}

/// The crosshair's minimum and maximum perturb (A3).
pub fn perturb_limits(p: &Perturb, mods: &PerturbMods, s: Stance) -> (i32, i32) {
    let mut min = (p.min as f32 * (1.0 + mods.upgrade)).trunc() as i32;
    let mut max = (p.max as f32 * (1.0 + mods.upgrade) * mods.set_rate).trunc() as i32;
    let (s_min, s_max) = mods.sums();
    if s.zoomed {
        if p.zoomed_max > 0 {
            max = p.zoomed_max + s_max;
        }
        if p.zoomed_min > 0 {
            min = p.zoomed_min + s_min;
        }
    } else if s.burst {
        if p.burst_max > 0 {
            max = p.burst_max;
        }
        if p.burst_min > 0 {
            min = p.burst_min;
        }
    }
    if p.duck > 0 && p.move_duck > 0 && s.crouch {
        min = if s.moving { p.move_duck } else { p.duck };
    }
    if let Some(pct) = mods.scope_min {
        min = (min as f32 + min as f32 * pct).trunc() as i32;
    }
    (min, max)
}

/// The spread gap g, px (A2): r is not clamped.
pub fn gap(min: i32, max: i32, r: f32, xr: f32) -> f32 {
    (min as f32 + r * (max - min) as f32) * xr
}

/// The firing kick (A, O), px, `since` seconds after the last shot (A4).
pub fn kick(since: Option<f32>, xr: f32) -> (f32, f32) {
    match since {
        Some(t) if (0.0..KICK_TIME).contains(&t) => {
            let u = t / KICK_TIME;
            (KICK_LENGTH * (1.0 - u) * xr, KICK_OFFSET * (1.0 - u) * xr)
        }
        _ => (0.0, 0.0),
    }
}

/// One thing the crosshair draws, px, 1 px thick.
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    /// An axis-aligned rect (the arms and the dot).
    Rect(PxRect),
    /// A 1-px line.
    Line(Vec2, Vec2),
    /// A 32-segment circle (`circle_vertices`).
    Circle { centre: Vec2, radius: f32 },
}

/// A circle's 32 vertices, from straight up, clockwise on screen.
pub fn circle_vertices(centre: Vec2, radius: f32) -> [Vec2; 32] {
    std::array::from_fn(|k| {
        let a = k as f32 * TAU / 32.0;
        centre + Vec2::new(a.sin(), -a.cos()) * radius
    })
}

/// The pieces of a shape (A5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    Left,
    Right,
    Top,
    Bottom,
    Dot,
    Diagonals,
    Circle,
    Brackets,
}

fn pieces(shape: u8) -> &'static [Piece] {
    use Piece::*;
    match shape {
        1 => &[Left, Right, Top, Bottom, Dot],
        2 => &[Left, Right, Bottom, Dot],
        3 => &[Diagonals, Dot],
        4 => &[Circle, Dot],
        5 => &[Circle],
        6 => &[Brackets, Dot],
        _ => &[Left, Right, Top, Bottom],
    }
}

/// The crosshair's measures for one frame (A2, A4), px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measures {
    pub centre: Vec2,
    /// The gap g, the kick (A, O), the arms' offset D and length l.
    pub g: f32,
    pub a: f32,
    pub o: f32,
    pub d: f32,
    pub l: f32,
    /// L0 x_r.
    pub l0: f32,
}

impl Measures {
    /// From the limits, r, the seconds since the shot, the weapon's
    /// offset (px, unscaled) and the screen.
    pub fn new(min: i32, max: i32, r: f32, since_shot: Option<f32>, offset: i32, screen: Screen) -> Self {
        let xr = screen.xr();
        let g = gap(min, max, r, xr);
        let (a, o) = kick(since_shot, xr);
        Self {
            centre: screen.centre(),
            g,
            a,
            o,
            d: g + o + offset as f32,
            l: ARM_LENGTH * xr + a,
            l0: ARM_LENGTH * xr,
        }
    }
}

/// The elements of `shape` (A5) with measures `m`.
pub fn shape_elements(shape: u8, m: &Measures) -> Vec<Element> {
    let (cx, cy) = (m.centre.x, m.centre.y);
    let (d, l) = (m.d, m.l);
    let mut out = Vec::new();
    for piece in pieces(shape) {
        match piece {
            Piece::Left => out.push(Element::Rect(PxRect::new(cx - d - 2.0 - l, cx - d - 2.0, cy - 1.0, cy))),
            Piece::Right => out.push(Element::Rect(PxRect::new(cx + d + 1.0, cx + d + 1.0 + l, cy - 1.0, cy))),
            Piece::Top => out.push(Element::Rect(PxRect::new(cx - 1.0, cx, cy - d - 2.0 - l, cy - d - 2.0))),
            Piece::Bottom => out.push(Element::Rect(PxRect::new(cx - 1.0, cx, cy + d + 1.0, cy + d + 1.0 + l))),
            Piece::Dot => out.push(Element::Rect(PxRect::new(cx - 1.0, cx, cy - 1.0, cy))),
            Piece::Diagonals => {
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let dir = Vec2::new(sx, sy) * FRAC_1_SQRT_2;
                    out.push(Element::Line(
                        m.centre + dir * (d + 2.0),
                        m.centre + dir * (d + 2.0 + l),
                    ));
                }
            }
            Piece::Circle => out.push(Element::Circle {
                centre: m.centre,
                radius: d + m.a + 1.0,
            }),
            Piece::Brackets => {
                let reach = 2.0 * m.l0 + m.g + m.a + 1.0;
                let arm = d + m.a + m.l0 + 2.0;
                let k = arm * FRAC_1_SQRT_2;
                for side in [-1.0, 1.0] {
                    let apex = Vec2::new(cx + side * reach, cy);
                    for up in [-1.0, 1.0] {
                        out.push(Element::Line(apex, apex + Vec2::new(-side * k, up * k)));
                    }
                }
            }
        }
    }
    out
}

/// One frame's crosshair inputs past A3: the weapon's own shape
/// (`CrossHairType`), the shape chosen (A6), the limits and r, the
/// seconds since the last shot and the last alternate shot, the weapon's
/// offset and wide-fire count.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrosshairInputs {
    pub weapon_shape: u8,
    pub shape: u8,
    pub min: i32,
    pub max: i32,
    pub r: f32,
    pub since_fire: Option<f32>,
    pub since_alt: Option<f32>,
    pub offset: i32,
    pub wide: i32,
}

/// The crosshair's measures and elements (A2 to A5), with the double
/// circle's second ring when the weapon's own shape is 5.
pub fn crosshair_elements(c: &CrosshairInputs, screen: Screen) -> (Measures, Vec<Element>) {
    let m = Measures::new(c.min, c.max, c.r, c.since_fire, c.offset, screen);
    let mut elements = shape_elements(c.shape, &m);
    if c.weapon_shape == 5 {
        let (min, max) = (c.min + c.min * c.wide, c.max + c.max / 2);
        let m2 = Measures::new(min, max, c.r, c.since_alt, c.offset, screen);
        elements.push(Element::Circle {
            centre: m2.centre,
            radius: m2.d + m2.a + 1.0,
        });
    }
    (m, elements)
}

/// The shape to draw (A6): the weapon's 1-6, else the crosshair item's
/// 1-6 (unless the room allows only tournament items and the item is not
/// on its list), else 0.
pub fn choose_shape(weapon: u8, item: Option<u8>, item_allowed: bool) -> u8 {
    if (1..=6).contains(&weapon) {
        weapon
    } else if let Some(s) = item.filter(|s| (1..=6).contains(s) && item_allowed) {
        s
    } else {
        0
    }
}

/// The colour's sources (A6), first match wins.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColourInputs {
    pub hit: bool,
    /// Dot-sight mode engaged with a dot-sight scope of a valid colour.
    pub dot_sight: Option<Rgba>,
    /// The crosshair-colour item's colour (0 counts as none).
    pub item: Option<Rgba>,
    /// The weapon's data exists, and its `CrossHairColor`.
    pub weapon: Option<Option<[u8; 3]>>,
}

/// This frame's colour (A6); `previous` stays without weapon data
/// (quirk 14).
pub fn choose_colour(c: ColourInputs, previous: Rgba) -> Rgba {
    if c.hit {
        HIT_CROSSHAIR
    } else if let Some(d) = c.dot_sight {
        d
    } else if let Some(i) = c.item.filter(|i| i[..3] != [0, 0, 0]) {
        i
    } else if let Some(w) = c.weapon {
        w.map_or(DEFAULT_CROSSHAIR, |[r, g, b]| [r, g, b, 255])
    } else {
        previous
    }
}

/// Whether the crosshair item draws (A7); `ok` is "no video, not frozen,
/// toggled on".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Visibility {
    /// No cutscene video, not AI-frozen, crosshair toggled on.
    pub ok: bool,
    pub scope_overlay: bool,
    /// Dead in the free-look or chase death camera.
    pub death_cam: bool,
    pub weapon: bool,
    pub disabled: bool,
    /// The weapon has a dot-sight image.
    pub dot_sight: bool,
    /// The fitted scope has a range readout.
    pub range_readout: bool,
}

/// What the crosshair item draws this frame (the render order of
/// "Per-frame order", step 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Draws {
    /// The damage overlay and direction indicator (when there is one).
    pub damage: bool,
    pub lines: bool,
    /// Times the dot-sight image is drawn (0 to 2, quirk 10).
    pub dot_sight: u8,
    pub range_readout: bool,
}

pub fn draws(v: Visibility) -> Draws {
    let mut d = Draws::default();
    if !v.ok {
        return d;
    }
    d.damage = true;
    if v.scope_overlay {
        d.range_readout = v.range_readout;
        return d;
    }
    if v.death_cam || !v.weapon {
        return d;
    }
    d.dot_sight = u8::from(v.dot_sight);
    if v.disabled {
        return d;
    }
    d.lines = true;
    d.range_readout = v.range_readout;
    d.dot_sight += u8::from(v.dot_sight);
    d
}

/// The infected's screen texture's rect (game_modes.md Corrections
/// (2026-10-30) 3): `pos` and `size` in 1024 x 768 units, x and y scaled
/// on their own (as A8).
pub fn infected_rect(screen: Screen, pos: [i32; 2], size: [i32; 2]) -> PxRect {
    let (xr, yr) = (screen.xr(), screen.yr());
    let (x, y) = (pos[0] as f32 * xr, pos[1] as f32 * yr);
    PxRect::new(x, x + size[0] as f32 * xr, y, y + size[1] as f32 * yr)
}

/// The dot-sight image's rect (A8), px.
pub fn dot_sight_rect(screen: Screen) -> PxRect {
    let (xr, yr) = (screen.xr(), screen.yr());
    let x = ((512.0 - 64.0) * xr).floor();
    let y = ((384.0 - 64.0) * yr).floor();
    let w = (128.0 * xr).floor() + 1.0;
    let h = (128.0 * yr).floor() + 1.0;
    PxRect::new(x, x + w, y, y + h)
}

/// The scope range readout (A9): the text and its colour for a hit at
/// `hit` units (None: nothing within the target range), with the
/// weapon's `range` and the attachments' range percentages summed; a hit
/// whose distance truncates to 0 shows nothing either.
pub fn range_readout(hit: Option<f32>, range: f32, range_pct: f32) -> (String, Rgba) {
    let Some(d) = hit.map(f32::trunc).filter(|d| *d >= 1.0) else {
        return (String::new(), RANGE_IN);
    };
    let eff = range + (range * range_pct).trunc();
    let colour = if d <= eff { RANGE_IN } else { RANGE_OUT };
    (format!("{:.1}", d / RANGE_DIVISOR), colour)
}

/// The hit indicator (A10): the latest hit of the local player's own
/// shot test on an enemy outside its spawn window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitIndicator {
    pub last: Option<f64>,
}

impl HitIndicator {
    /// The local player's shot trace hit another player's hit box at
    /// `now` ms (`lights`: an enemy outside its spawn window).
    pub fn hit(&mut self, now: f64, lights: bool) {
        if lights {
            self.last = Some(now);
        }
    }
    /// A HUD frame at `now` ms: off once more than 500 ms have passed.
    pub fn frame(&mut self, now: f64) -> bool {
        if self.last.is_some_and(|t| now - t > HIT_TIME_MS) {
            self.last = None;
        }
        self.last.is_some()
    }
    /// The round ended.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// Whether a traced hit lights the hit indicator (A10): the victim's team
/// id differs from the local player's and its spawn window is shut.
pub fn hit_lights(enemy: bool, window_open: bool) -> bool {
    enemy && !window_open
}

/// A player's no-damage window as this client sees it (A10, Corrections
/// 1): open from its (re)spawn on this client for 3500 OS ms (shut on the
/// first frame more than that later), or until its first shot after it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpawnWindow {
    /// When it (re)spawned, OS ms.
    pub spawned: Option<f64>,
    /// It fired since.
    pub fired: bool,
}

impl SpawnWindow {
    /// It (re)spawned at `now` ms.
    pub fn spawn(&mut self, now: f64) {
        self.spawned = Some(now);
        self.fired = false;
    }
    /// It fired.
    pub fn fire(&mut self) {
        self.fired = true;
    }
    /// Open at `now` ms.
    pub fn open(&self, now: f64) -> bool {
        !self.fired && self.spawned.is_some_and(|t| now - t <= SPAWN_WINDOW_MS)
    }
}

// ---------------------------------------------------------------------------
// Name tags (A11)

/// Whether a tag whose anchor is `depth` ahead of the camera (view-space
/// depth) is drawn: no distance limit, only one behind the camera hides
/// (Open question 10).
pub fn tag_in_front(depth: f32) -> bool {
    depth > 0.0
}

/// Who a tag is for, as the local player sees them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagInputs {
    pub local_alive: bool,
    pub teammate: bool,
    pub alive: bool,
    /// The local player's aim target.
    pub aimed: bool,
    /// Carries the flag or is revealed by a scanner.
    pub revealed: bool,
    /// Hit by the local player within the last 0.2 s.
    pub flashing: bool,
    /// The camera sees the tag point.
    pub seen: bool,
    /// Followed by the chase camera (the watched player, in the death
    /// camera too).
    pub chased: bool,
    /// Dead in Hunted: every other player's tag shows
    /// (death_killcam.md 5).
    pub everyone: bool,
    /// A "state viewer" weapon or item.
    pub state_viewer: bool,
    /// Dead: its body is there and visible (the tag floats over it).
    pub body_visible: bool,
    /// Carries a flag.
    pub flag: bool,
    /// One of the local player's revenge targets.
    pub revenge: bool,
}

/// A shown tag: the name's colour, the tag's opacity, the HP bar, and the
/// icons and numbers it holds (Corrections 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: Rgba,
    pub opacity: u8,
    pub hp_bar: bool,
    /// The player's rank icon (its "alive" variant).
    pub rank: bool,
    pub flag: bool,
    pub revenge: bool,
    /// The kill/death ratio (a state viewer).
    pub kd: bool,
}

/// A player's name tag (A11; never called for the local player): a dead
/// player's for everyone, while its body is visible (Corrections 5).
pub fn name_tag(t: TagInputs) -> Option<Tag> {
    let shown = if !t.alive {
        t.body_visible
    } else if t.local_alive {
        t.teammate || t.aimed || t.revealed
    } else {
        t.teammate || t.aimed || t.chased || t.everyone
    };
    if !shown {
        return None;
    }
    let name = if !t.alive {
        TAG_DEAD
    } else if t.teammate {
        TAG_MATE
    } else if t.flashing {
        TAG_FLASH
    } else {
        TAG_ENEMY
    };
    Some(Tag {
        name,
        opacity: if t.seen { TAG_SEEN } else { TAG_OCCLUDED },
        hp_bar: t.aimed && t.alive && (t.teammate || t.state_viewer),
        rank: t.aimed && t.alive,
        flag: t.flag,
        revenge: t.revenge && !t.teammate,
        kd: t.state_viewer,
    })
}

// ---------------------------------------------------------------------------
// Damage taken (C)

/// The damage overlay and direction indicator: the latest hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageFeedback {
    /// When it arrived, ms.
    pub at: f64,
    pub alpha: u8,
    /// The continuous sector s (3 front, 0 or 12 right, 6 left, 9 behind).
    pub s: f32,
    /// The player's yaw at the hit (radians, growing to the right).
    pub yaw: f32,
}

impl DamageFeedback {
    /// A hit at `now` ms that travelled along `d` (source to player),
    /// with the forward `f` of the player's view rotation (pitch and yaw:
    /// it tilts with the pitch) and its horizontal right `rt`, and its yaw
    /// then (C). None: no health was lost (nothing is shown). The first
    /// frame's fade (to 253) is the caller's `frame` on the same frame.
    pub fn hit(now: f64, loss: f32, f: Vec3, rt: Vec3, d: Vec3, yaw: f32) -> Option<Self> {
        if loss <= 0.0 {
            return None;
        }
        let theta = PI + f.dot(d).atan2(rt.dot(d));
        Some(Self {
            at: now,
            alpha: 255,
            s: theta / (PI / 6.0),
            yaw,
        })
    }

    /// A HUD frame at `now` ms, the hit's own included (Corrections 3:
    /// it draws 253): fade by 2 (floor 0); false once 1000 ms have passed
    /// (it goes).
    pub fn frame(&mut self, now: f64) -> bool {
        if now - self.at >= DAMAGE_LIFE_MS {
            return false;
        }
        self.alpha = self.alpha.saturating_sub(DAMAGE_ALPHA_STEP);
        true
    }

    /// The indicator's rotation for the player's yaw now.
    pub fn phi(&self, yaw_now: f32) -> f32 {
        PI * (SECTOR_FACTOR * (self.s - 3.0) + 0.25) + (yaw_now - self.yaw)
    }

    /// The indicator's corners P0..P3 (they carry the texture's top-left,
    /// top-right, bottom-right and bottom-left), px.
    pub fn corners(&self, yaw_now: f32, screen: Screen) -> [Vec2; 4] {
        let phi = self.phi(yaw_now);
        let r = INDICATOR_SIDE * FRAC_1_SQRT_2;
        std::array::from_fn(|k| {
            let a = -(phi + k as f32 * PI / 2.0);
            Vec2::new((512.0 + r * a.cos()) * screen.xr(), (384.0 + r * a.sin()) * screen.yr())
        })
    }
}

/// The hit overlay's rect: 1024 x 1024 ref from the top left (quirk 5).
pub fn hit_overlay_rect(screen: Screen) -> PxRect {
    PxRect::new(0.0, (REF_W * screen.xr()).floor(), 0.0, (REF_W * screen.yr()).floor())
}

/// The indicator's texture coordinates (u, v ranges; v runs past 1,
/// quirk 6).
pub fn indicator_uv() -> (f32, f32) {
    (INDICATOR_SIDE / INDICATOR_TEXTURE.x, INDICATOR_SIDE / INDICATOR_TEXTURE.y)
}

// ---------------------------------------------------------------------------
// Health, armour, stamina (D2, D3)

/// The HP value shown, for health `h` of `max` and the model's
/// `InterfaceMaxHealth` `interface`.
pub fn hp_shown(h: f32, max: f32, interface: f32) -> i32 {
    if h > 0.0 {
        (h * interface / max).max(1.0).floor() as i32
    } else {
        0
    }
}

/// The AP value shown.
pub fn ap_shown(armour: f32) -> i32 {
    armour.max(0.0).trunc() as i32
}

/// Low health: floor(100 h / h_max) <= 30, not infected.
pub fn low_health(h: f32, max: f32, infected: bool) -> bool {
    !infected && (100.0 * h / max).floor() as i32 <= LOW_HEALTH_PERCENT
}

/// The heartbeat timer (D2, Corrections 4): started, silently, on the
/// first low-health frame of the session and never reset; a beat on a low
/// frame more than 1000 OS ms after its last restart, which restarts it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Heartbeat {
    pub last: Option<f64>,
}

impl Heartbeat {
    /// A HUD frame at `now` OS ms: true when a heartbeat plays.
    pub fn frame(&mut self, now: f64, low: bool) -> bool {
        if !low {
            return false;
        }
        match self.last {
            None => {
                self.last = Some(now);
                false
            }
            Some(t) if now - t > HEARTBEAT_MS => {
                self.last = Some(now);
                true
            }
            Some(_) => false,
        }
    }
}

/// The colours of the health block (D2): the HP and AP values, the "HP"
/// and "AP" labels, the silhouette tint (none: untinted), the "SP" and
/// set labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HealthColours {
    pub values: Rgba,
    pub hp_ap_labels: Rgba,
    pub silhouette: Option<Rgba>,
    pub other_labels: Rgba,
}

pub const VALUE_WHITE: Rgba = [255, 255, 255, 255];
pub const LABEL_GREY: Rgba = [216, 216, 216, 255];

pub fn health_colours(low: bool) -> HealthColours {
    HealthColours {
        values: if low { LOW_HEALTH } else { VALUE_WHITE },
        hp_ap_labels: if low { LOW_HEALTH } else { LABEL_GREY },
        silhouette: low.then_some(LOW_HEALTH),
        other_labels: LABEL_GREY,
    }
}

/// The stance silhouette: crouched at a crouch blend of 0.5 or more.
pub fn crouch_image(blend: f32) -> bool {
    blend >= 0.5
}

/// The stamina bar's width, ref px (D3; 0: hidden).
pub fn stamina_bar(stamina: f32, max: f32) -> f32 {
    if max <= 0.0 {
        return 0.0;
    }
    let fill = ((stamina / max) * 100.0).floor().clamp(0.0, 100.0);
    (fill * 120.0 / 100.0).floor()
}

// ---------------------------------------------------------------------------
// The weapon panel (D4)

/// A number as the panel prints it: hundreds, tens and ones concatenated.
pub fn three_digits(n: u32) -> String {
    format!("{}{}{}", n / 100, (n / 10) % 10, n % 10)
}

/// The fire-mode icon (D4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireIcon {
    Burst2,
    Burst3,
    Auto,
    Single,
}

/// The fire-mode icon (D4, Open question 16) of a weapon of inventory
/// class `class` (1 primary, 2 secondary: the others have none): in
/// burst, by its round count (`TripleShotRoundNum`); else by
/// `RapidFireCount`: 1 (semi-automatic) single shot, or full auto while
/// its continuous fire is on; 0 full auto; any other count none.
pub fn fire_icon(class: u8, burst: Option<u32>, rapid_fire_count: u32, continuous: bool) -> Option<FireIcon> {
    if !matches!(class, 1 | 2) {
        return None;
    }
    match (burst, rapid_fire_count) {
        (Some(2), _) => Some(FireIcon::Burst2),
        (Some(_), _) => Some(FireIcon::Burst3),
        (None, 1) if continuous => Some(FireIcon::Auto),
        (None, 1) => Some(FireIcon::Single),
        (None, 0) => Some(FireIcon::Auto),
        _ => None,
    }
}

/// What the panel shows of a weapon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponPanel {
    pub name: String,
    /// Clip and reserve texts; none for infinite-ammo weapons.
    pub ammo: Option<(String, String)>,
    pub icon: Option<FireIcon>,
}

/// The panel's inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelInputs {
    pub name: String,
    /// The weapon's inventory class (attribute_data.md: the order list it
    /// is in, 1 primary ... 6 mission; 0 in none).
    pub class: u8,
    /// The mission bomb (`Guntype` 8).
    pub bomb: bool,
    pub infinite: bool,
    /// Clip and the total held for the weapon (the reserve is the rest).
    pub clip: u32,
    pub total: u32,
    /// In burst mode: its round count.
    pub burst: Option<u32>,
    /// `RapidFireCount`, and whether its continuous fire is on.
    pub rapid_fire_count: u32,
    pub continuous: bool,
}

/// The panel (None: hidden; `shown` is D2's visibility and not infected):
/// for inventory classes 1 to 6 but the mission bomb.
pub fn weapon_panel(p: &PanelInputs, shown: bool) -> Option<WeaponPanel> {
    if !shown || !(1..=6).contains(&p.class) || p.bomb {
        return None;
    }
    if p.infinite {
        return Some(WeaponPanel {
            name: p.name.clone(),
            ammo: None,
            icon: None,
        });
    }
    Some(WeaponPanel {
        name: p.name.clone(),
        ammo: Some((three_digits(p.clip), three_digits(p.total.saturating_sub(p.clip)))),
        icon: fire_icon(p.class, p.burst, p.rapid_fire_count, p.continuous),
    })
}

/// The panel's places and colours, ref.
pub mod panel {
    use super::Rgba;
    use bevy::math::Vec2;

    pub const NAME_BOX: (Vec2, Vec2) = (Vec2::new(769.0, 716.0), Vec2::new(250.0, 10.0));
    pub const NAME: Rgba = [222, 222, 222, 255];
    pub const CLIP_AT: Vec2 = Vec2::new(910.0, 742.0);
    pub const CLIP: Rgba = [240, 240, 240, 255];
    pub const SLASH_AT: Vec2 = Vec2::new(962.0, 742.0);
    pub const RESERVE_AT: Vec2 = Vec2::new(974.0, 741.0);
    pub const RESERVE: Rgba = [180, 180, 180, 255];
    pub const ICON: (Vec2, Vec2) = (Vec2::new(926.0, 731.0), Vec2::new(128.0, 16.0));
}

/// The health block's places, ref (D2, D3).
pub mod status {
    use bevy::math::Vec2;

    pub const PANEL: (Vec2, Vec2) = (Vec2::new(-7.0, 717.0), Vec2::new(238.0, 49.0));
    pub const PLATES: [(Vec2, Vec2); 4] = [
        (Vec2::new(67.0, 728.0), Vec2::new(76.0, 16.0)),
        (Vec2::new(150.0, 728.0), Vec2::new(76.0, 16.0)),
        (Vec2::new(67.0, 747.0), Vec2::new(159.0, 16.0)),
        (Vec2::new(2.0, 747.0), Vec2::new(62.0, 16.0)),
    ];
    pub const HP_LABEL: Vec2 = Vec2::new(72.0, 728.0);
    pub const AP_LABEL: Vec2 = Vec2::new(155.0, 728.0);
    pub const SP_LABEL: Vec2 = Vec2::new(72.0, 747.0);
    pub const SET_LABEL: Vec2 = Vec2::new(5.0, 747.0);
    pub const HP_BOX: (Vec2, Vec2) = (Vec2::new(97.0, 721.0), Vec2::new(40.0, 30.0));
    pub const AP_BOX: (Vec2, Vec2) = (Vec2::new(180.0, 721.0), Vec2::new(40.0, 30.0));
    pub const SILHOUETTE: (Vec2, Vec2) = (Vec2::new(0.0, 654.0), Vec2::new(64.0, 128.0));
    pub const STAMINA_GAUGE: (Vec2, Vec2) = (Vec2::new(97.0, 751.0), Vec2::new(124.0, 16.0));
    pub const STAMINA_BAR: Vec2 = Vec2::new(99.0, 753.0);
    pub const STAMINA_BAR_HEIGHT: f32 = 16.0;
}

// ---------------------------------------------------------------------------
// Kill feed (D5)

/// The weapon slot's picture on a kill line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KillIcon {
    /// The weapon's icon (by its id); none: the slot is empty but kept.
    Weapon(Option<String>),
    Suicide,
    Barrel,
    Fire,
    Poison,
    Bleeding,
    Npc,
}

/// A kill line's badges, in the order they are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Badge {
    Revenge,
    Headshot,
    Groin,
    /// The flag carrier, the enemy-flag image: a victim on the local
    /// player's team (it carried the enemy's flag).
    Flag,
    /// The flag carrier, the team-flag image: a victim on another team.
    TeamFlag,
    Gold,
    IceCrush,
    Vip,
    VipRank,
}

/// How a death came about, as the kill line's weapon slot reads it (D5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathKind {
    /// Crush damage (an ordinary fall is crush) or a fall out of the
    /// world.
    Crush,
    /// An explosion or a bump.
    Blast,
    Burn,
    Poison,
    Bleeding,
    Other,
}

/// The weapon slot's picture (D5, Corrections 7): the suicide icon for a
/// crush death; with no killer named the barrel, fire, poison or bleeding
/// icon (another kind keeps the message's weapon icon); an AI killer the
/// NPC icon; else the weapon's own icon, a self-kill's too.
pub fn kill_icon(named: bool, npc: bool, kind: DeathKind, weapon: Option<String>) -> KillIcon {
    match (kind, named) {
        (DeathKind::Crush, _) => KillIcon::Suicide,
        (DeathKind::Blast, false) => KillIcon::Barrel,
        (DeathKind::Burn, false) => KillIcon::Fire,
        (DeathKind::Poison, false) => KillIcon::Poison,
        (DeathKind::Bleeding, false) => KillIcon::Bleeding,
        _ if npc => KillIcon::Npc,
        _ => KillIcon::Weapon(weapon),
    }
}

/// The badges of a kill line (D5's order): revenge, headshot (whenever
/// the killing hit was on the head, killer named or not), groin shot, flag carrier
/// (the enemy-flag image for a victim on the local player's team, the
/// team-flag image otherwise).
pub fn kill_badges(revenge: bool, head: bool, groin: bool, flag_carrier: bool, victim_mate: bool) -> Vec<Badge> {
    let mut out = Vec::new();
    if revenge {
        out.push(Badge::Revenge);
    }
    if head {
        out.push(Badge::Headshot);
    }
    if groin {
        out.push(Badge::Groin);
    }
    if flag_carrier {
        out.push(if victim_mate { Badge::Flag } else { Badge::TeamFlag });
    }
    out
}

/// One kill message.
#[derive(Clone, Debug, PartialEq)]
pub struct Kill {
    /// None: no killer (the world). A self-kill names the victim.
    pub killer: Option<(String, bool)>,
    /// The victim's name and whether on the local player's team.
    pub victim: (String, bool),
    pub icon: KillIcon,
    pub badges: Vec<Badge>,
}

/// A text's colour on a kill line.
pub fn feed_colour(mate: bool) -> Rgba {
    let [r, g, b] = if mate { FEED_MATE } else { FEED_OTHER };
    [r, g, b, FEED_TEXT_ALPHA]
}

/// One placed element of a kill line (ref, x from the line's left).
#[derive(Clone, Debug, PartialEq)]
pub enum FeedItem {
    Text { x: f32, text: String, colour: Rgba },
    Icon { x: f32, icon: KillIcon },
    Badge { x: f32, badge: Badge },
}

/// A line's elements left to right (D5), `width` the rendered width of a
/// name (ref px).
pub fn feed_line(k: &Kill, width: impl Fn(&str) -> f32) -> Vec<FeedItem> {
    let mut out = Vec::new();
    let mut x = FEED_LEFT;
    let (killer, mate) = k.killer.clone().unwrap_or_else(|| k.victim.clone());
    out.push(FeedItem::Text {
        x,
        text: killer.clone(),
        colour: feed_colour(mate),
    });
    x += width(&killer);
    if let KillIcon::Weapon(None) = k.icon {
    } else {
        out.push(FeedItem::Icon { x, icon: k.icon.clone() });
    }
    x += FEED_ICON_WIDTH;
    // The first badge takes the first slot, every later one the second,
    // moved to the running x each time (quirk 12, Corrections 6); x moves
    // on for each.
    let mut slots: [Option<(f32, Badge)>; 2] = [None, None];
    for (i, b) in k.badges.iter().enumerate() {
        slots[usize::from(i > 0)] = Some((x, *b));
        x += FEED_BADGE_WIDTH;
    }
    for (at, badge) in slots.into_iter().flatten() {
        out.push(FeedItem::Badge { x: at, badge });
    }
    out.push(FeedItem::Text {
        x,
        text: k.victim.0.clone(),
        colour: feed_colour(k.victim.1),
    });
    out
}

/// The kill feed: at most five lines, each 30 s.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KillFeed {
    pub lines: VecDeque<(f64, Kill)>,
}

impl KillFeed {
    /// A kill message at `now` ms.
    pub fn push(&mut self, now: f64, kill: Kill) {
        if self.lines.len() == FEED_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back((now, kill));
    }
    /// A HUD frame at `now` ms: lines older than their life (more than
    /// 30000 ms) go, the rest close up.
    pub fn frame(&mut self, now: f64) {
        self.lines.retain(|(t, _)| now - *t <= FEED_LIFE_MS);
    }
    /// Line `i`'s top (0 the oldest shown), ref.
    pub fn line_y(i: usize) -> f32 {
        FEED_TOP + FEED_LINE_HEIGHT * i as f32
    }
}

// ---------------------------------------------------------------------------
// Score panel and round timer (D6, D7)

/// The team deathmatch panel's highlight colours by team (Alpha, Bravo),
/// and the plain colour.
pub const SCORE_PLAIN: Rgba = [170, 170, 170, 255];
pub const SCORE_ALPHA: Rgba = [223, 31, 20, 255];
pub const SCORE_BRAVO: Rgba = [13, 109, 255, 255];

/// A team's progress bar width, ref px (0: hidden).
pub fn score_bar(kills: u32, goal: u32) -> f32 {
    if kills == 0 || goal == 0 {
        return 0.0;
    }
    let gauge = (100.0 * kills as f32 / goal as f32).min(100.0);
    (gauge * 155.0 / 100.0).floor()
}

/// One team row of the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreRow {
    pub score: String,
    pub colour: Rgba,
    pub bar: f32,
}

/// The panel's rows (Alpha, Bravo) and the highlight's top-left (ref)
/// for the local player's team (0 Alpha, 1 Bravo, none: no highlight).
pub fn score_panel(kills: [u32; 2], goal: u32, mine: Option<usize>) -> ([ScoreRow; 2], Option<Vec2>) {
    let rows = std::array::from_fn(|i| ScoreRow {
        score: kills[i].to_string(),
        colour: match (mine == Some(i), i) {
            (true, 0) => SCORE_ALPHA,
            (true, _) => SCORE_BRAVO,
            _ => SCORE_PLAIN,
        },
        bar: score_bar(kills[i], goal),
    });
    let highlight = mine.map(|i| if i == 0 { Vec2::new(344.0, 18.0) } else { Vec2::new(344.0, 26.0) });
    (rows, highlight)
}

/// The round timer (D7): round-based modes only (team deathmatch has
/// none).
pub fn timer(round_based: bool, ms: f64) -> Option<String> {
    round_based.then(|| round_timer(ms))
}

/// The round timer's text for `ms` remaining (D7): "MM : SS", truncated,
/// stopping at "00 : 00".
pub fn round_timer(ms: f64) -> String {
    let t = ms.max(0.0).trunc() as u64;
    format!("{:02} : {:02}", t / 60000, (t % 60000) / 1000)
}

/// Whether game mode `key` is a round mode on the HUD (game_modes.md
/// S13): its goal is rounds and its panel counts round wins.
pub fn round_mode(key: &str) -> bool {
    matches!(
        key,
        "TMM" | "CMM" | "QRT" | "TSV" | "CTSV" | "LMS" | "SBT" | "SQM" | "ESL" | "ESC" | "CESC" | "HG"
    )
}

/// Whether mode `key`'s panel row titles show each team's living players
/// (D6, Corrections 9: Elimination Pro, Search & Destroy, Bombing Run).
pub fn rows_count_living(key: &str) -> bool {
    matches!(key, "TSV" | "CTSV" | "TMM" | "CMM" | "SBT")
}

/// The goal text's string-table entry for mode `key` (game_modes.md S13,
/// Corrections 1): rounds, kills or minutes, each a format the goal fills;
/// None: the goal, a space and `IDS_POINTS`.
pub fn goal_string(key: &str) -> Option<&'static str> {
    match key {
        "CTF" | "CCTF" | "SQM2" | "VTDM" => None,
        "BNT" | "HQM" => Some("IDS_GAMEROOM_MIN"),
        k if round_mode(k) => Some("IDS_GAMEROOM_ROUND"),
        _ => Some("IDS_GAMEROOM_KILL"),
    }
}

/// The goal text for mode `key` and goal `goal`, the strings by `resolve`
/// (a format's first `%d` or `%s` filled with the goal).
pub fn goal_text(key: &str, goal: u32, resolve: impl Fn(&str) -> String) -> String {
    match goal_string(key) {
        Some(id) => fill(&resolve(id), &goal.to_string()),
        None => format!("{goal} {}", resolve("IDS_POINTS")),
    }
}

/// A format's first insert filled with `with` (none: the format).
pub fn fill(format: &str, with: &str) -> String {
    fill_all(format, &[with])
}

/// A format with its inserts filled from `with`: the install's
/// FormatMessage style (`%1`, `%2!d!`, `%1!s!`: argument N, 1-based, the
/// optional `!type!` after it dropped) and the plain `%d` / `%s` (the
/// next argument in order); `%%` is a `%`. An insert with no argument
/// stays as written.
pub fn fill_all(format: &str, with: &[&str]) -> String {
    let mut out = String::with_capacity(format.len());
    let mut next = 0;
    let mut rest = format;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        let (arg, used) = if tail.starts_with('%') {
            out.push('%');
            rest = &tail[1..];
            continue;
        } else if digits > 0 {
            let n: usize = tail[..digits].parse().unwrap_or(0);
            // `!type!` after the number.
            let spec = tail[digits..]
                .strip_prefix('!')
                .and_then(|t| t.find('!').map(|e| e + 2))
                .unwrap_or(0);
            (n.checked_sub(1), digits + spec)
        } else if tail.starts_with('d') || tail.starts_with('s') {
            next += 1;
            (Some(next - 1), 1)
        } else {
            out.push('%');
            rest = tail;
            continue;
        };
        match arg.and_then(|i| with.get(i)) {
            Some(w) => out.push_str(w),
            None => out.push_str(&rest[at..at + 1 + used]),
        }
        rest = &tail[used..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod fill_tests {
    use super::fill_all;

    #[test]
    fn inserts() {
        assert_eq!(fill_all("%1!d! Kills", &["100"]), "100 Kills");
        assert_eq!(fill_all("Goal: %1!s!", &["x"]), "Goal: x");
        assert_eq!(fill_all("%2 of %1!d!", &["3", "9"]), "9 of 3");
        assert_eq!(fill_all("%1 and %1", &["a"]), "a and a");
        assert_eq!(fill_all("100%% of %1", &["a"]), "100% of a");
        assert_eq!(fill_all("%1!d! / %2!d!", &["1"]), "1 / %2!d!");
        assert_eq!(fill_all("%d of %s", &["1", "2"]), "1 of 2");
        assert_eq!(fill_all("%d of %s", &["1"]), "1 of %s");
        assert_eq!(fill_all("50% off", &[]), "50% off");
        assert_eq!(fill_all("%0 %1!d", &["a"]), "%0 a!d", "an unclosed type stays");
    }
}

/// A round mode's bar frame (`at`, `size`, ref px) carries G - 1 divider
/// marks (D6, Corrections 9): each mark's top-left and height, at
/// x + 1 + trunc(i / G (w - 2)), y + 2, h - 2, for i = 1 .. G - 1.
pub fn round_dividers(at: Vec2, size: Vec2, goal: u32) -> Vec<(Vec2, f32)> {
    (1..goal.max(1))
        .map(|i| {
            let x = at.x + 1.0 + (i as f32 / goal as f32 * (size.x - 2.0)).trunc();
            (Vec2::new(x, at.y + 2.0), size.y - 2.0)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The action message and the action gauge (D9)

/// The action message (D9): the latest request's string id and when it
/// was made; shown until the first frame more than 100 OS ms after it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionMessage {
    pub current: Option<(String, f64)>,
}

impl ActionMessage {
    /// Ask for message `id` at `now` ms (made every frame its condition
    /// holds).
    pub fn request(&mut self, id: &str, now: f64) {
        self.current = Some((id.to_string(), now));
    }
    /// A HUD frame at `now` ms: the message shown, if any.
    pub fn frame(&mut self, now: f64) -> Option<&str> {
        if self.current.as_ref().is_some_and(|(_, t)| now - t > ACTION_MESSAGE_MS) {
            self.current = None;
        }
        self.current.as_ref().map(|(id, _)| id.as_str())
    }
}

/// The action message's x (D9): centred on `ActionDesc` (x, width) by its
/// text's reference width `w`: (x + ⌊width / 2⌋) − ⌊w / 2⌋.
pub fn message_x(desc_x: i32, desc_w: i32, w: i32) -> i32 {
    desc_x + desc_w.div_euclid(2) - w.div_euclid(2)
}

/// Why the pickup prompt may be asked for (D9): the crosshair's draw step
/// runs (no video or AI freeze, the crosshair item on, no scope overlay,
/// not dead in the free-look or chase camera, weapon data for the held
/// weapon) and the player may pick up (no mine in hand, not infected, a
/// human model, no weapon-replacing buff, not the escorted VIP, not
/// holding intel in Spy Hunt's second stage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupInputs {
    pub draw_step: bool,
    pub scope_overlay: bool,
    pub death_cam: bool,
    pub weapon_data: bool,
    pub aiming_at_pickup: bool,
    pub mine: bool,
    pub infected: bool,
    pub not_human: bool,
    pub buff: bool,
    pub vip: bool,
    pub spy_intel: bool,
}

/// The pickup prompt's string id, when asked for (D9; `own_bag`: Hunted,
/// aiming at one's own bag).
pub fn pickup_prompt(p: PickupInputs, own_bag: bool) -> Option<&'static str> {
    let shown = p.draw_step
        && !p.scope_overlay
        && !p.death_cam
        && p.weapon_data
        && p.aiming_at_pickup
        && !p.mine
        && !p.infected
        && !p.not_human
        && !p.buff
        && !p.vip
        && !p.spy_intel;
    shown.then_some(if own_bag { "IDS_HUD_Search_mybag" } else { "IDS_HUD_ITEMGET_MSG" })
}

/// The bomb prompts (D9): defuse while aiming at a bomb target the
/// player's side defuses with no defuse under way; plant while the player
/// may plant and isn't planting (the bomb in hand, else take it out).
pub fn bomb_prompt(defusable_aimed: bool, defusing: bool, may_plant: bool, planting: bool, bomb_in_hand: bool) -> Option<&'static str> {
    if defusable_aimed && !defusing {
        return Some("IDS_HUD_BOMBDEFUSE_MSG");
    }
    if may_plant && !planting {
        return Some(if bomb_in_hand { "IDS_HUD_BOMBSET_MSG" } else { "IDS_HUD_BOMBSELECT_MSG" });
    }
    None
}

/// What the action gauge shows (D9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GaugeKind {
    Plant,
    Defuse,
    /// An activate or recharge trigger (Seize and Secure's pole).
    Custom,
    TeamChange,
}

impl GaugeKind {
    /// Its title's string id (a custom gauge without its own: the
    /// default).
    pub fn title(self) -> &'static str {
        match self {
            Self::Plant => "$GaugeBombPlant",
            Self::Defuse => "$GaugeBombDefuse",
            Self::Custom => "$GaugeDefault",
            Self::TeamChange => "$TeamChange",
        }
    }
    /// Its icon's HUD layout attribute key (lower case; as the data
    /// spells it).
    pub fn icon_key(self) -> Option<&'static str> {
        match self {
            Self::Plant => Some("guage_bombplant_tex"),
            Self::Defuse => Some("guage_bombdefuse_tex"),
            Self::TeamChange => Some("guage_teamchagne_tex"),
            Self::Custom => None,
        }
    }
}

/// The action gauge's own timer (D9, Corrections 12): it starts from 0 on
/// the frame its state changes (game seconds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActionGauge {
    pub state: Option<(GaugeKind, f64)>,
}

impl ActionGauge {
    /// A frame at `now` s with the state `kind` lasting `total` s: the
    /// fill in whole percent, or None (no state, or its time is up: it
    /// hides, even while the key is held).
    pub fn frame(&mut self, kind: Option<GaugeKind>, total: f64, now: f64) -> Option<u32> {
        let Some(kind) = kind else {
            self.state = None;
            return None;
        };
        let since = match self.state {
            Some((k, t)) if k == kind => t,
            _ => {
                self.state = Some((kind, now));
                now
            }
        };
        gauge_percent(now - since, total)
    }
}

/// The gauge's fill after `elapsed` of `total` s: trunc(100 · elapsed /
/// total) whole percent; None once the elapsed time reaches the total.
pub fn gauge_percent(elapsed: f64, total: f64) -> Option<u32> {
    if total <= 0.0 || elapsed >= total {
        return None;
    }
    Some((100.0 * elapsed.max(0.0) / total + 1e-9).trunc() as u32)
}

/// A gauge's bar width for `p` percent of `max_range` (ui_system.md W5).
pub fn gauge_bar(p: u32, max_range: i32) -> i32 {
    (p as i64 * max_range as i64).div_euclid(100) as i32
}

// ---------------------------------------------------------------------------
// The gauge sight (A12, Corrections 13)

/// The gauge sight's two images: the HUD layout attributes'
/// `CrossHairLeftTex` and `CrossHairRightTex` prefixes with health's and
/// the clip's steps (`game::attachments::gauge_image`: (q + 5) − ((q + 5)
/// mod 10)) and ".dtx".
pub fn gauge_images(left: &str, right: &str, health_q: i32, clip_q: i32) -> (String, String) {
    use crate::game::attachments::gauge_image;
    (
        format!("{left}{}.dtx", gauge_image(health_q)),
        format!("{right}{}.dtx", gauge_image(clip_q)),
    )
}
