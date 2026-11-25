//! Counter-Strike 1.6's HUD while a CS 1.6 weapon is in hand
//! (specs/cs16/hud.md): health with the cross and its bar at the bottom
//! left, armour with the suit icon lit from the bottom by the armour left
//! (the helmet pair with a helmet), the ammunition at the bottom right
//! (clip, bar, reserve and the ammunition icon; a grenade's count alone),
//! the numbers in the number sprites, dim at rest and flaring on a change
//! (`Fades`), in the HUD colour (health in the warning colour at 25 or
//! less), and the damage direction arcs (`Pain`). Everything sits at its
//! sprites' own pixel size by the spec's formulas in the window's real
//! pixels (no scaling: parity). The sprites are the install's grey
//! additive atlases (`cs16::game::hud`), tinted and added onto the frame
//! (`HudGlyphMaterial`, additive). `load_pictures` also registers the
//! `d_` kill icons and the headshot mark for every app's kill feed.
//!
//! Also drawn (hud.md): the dynamic crosshair (four 1-pixel lines of
//! `cl_crosshair_color` around the centre: the weapon's base gap by its
//! movement rule (`Rule`, `base_gap`), opened a step per shot up to 15 and
//! closed 1.3 % + 0.1 pixel per drawn frame (`Fades::step`, frame rate
//! dependent as the spec keeps it), scaled by `cl_crosshair_size`; hidden
//! for the snipers and at a field of view of 40 or less; the shell's is
//! off), the weapon selection's bucket list while the shell's `WeaponMenu`
//! is open (`hud_fastswitch 0`, no timeout: `keep_open`), the round timer
//! and the 320 layout below 640 pixels wide (`resolution_for`: that
//! layout's own sprites; the digits' cell size sets the numbers' spacing,
//! as the spec's formulas have it).
//!
//! - `Frame`: what the HUD shows, from the local player's state;
//!   `draws`: the frame laid out as sprites and filled boxes in screen
//!   pixels (pure; tests read both without a window);
//! - the Bevy side: `spawn_root`, `collect` (the frame, the fades),
//!   `pain` (the arcs' strengths), `sync` (a pool of nodes from the
//!   draws) and `load_pictures`.
//!
//! Known shortcuts:
//! - Money's place and flash are not drawn (hud.md question 2; the party
//!   has no money).
//! - The pistols' speed rule starts at 0 as hud.md has it (question 3,
//!   still to confirm by a capture), and `hud_fastswitch 1` with several
//!   weapons in a slot switches to the first one instead of opening the
//!   menu or cycling (question 3).
//! - The weapon menu draws for the party's rungs as the player holds them
//!   (slots 1-5 always, the open slot's pictures); a grenade's ammunition
//!   bar is not drawn (only magazine weapons have a reserve to show), and
//!   the menu closes with the weapon's death or spectating, not on a round
//!   reset. The crosshair's night vision colour and the raised shield's
//!   hiding are not drawn (neither exists in the party).
//! - The round timer shows the match's time left (`Match::round_ends`,
//!   truncated to seconds) while the match is live, hidden without a limit
//!   or with more than a day left (the sandbox's), not the server's own
//!   hide message.
//! - The kill feed is the app's, with CS 1.6's `d_` icons tinted 255 80 0
//!   and blended by their brightness (not added), `d_skull` for a CS 1.6
//!   weapon without an icon; its layout and names are the app's.
//! - Not drawn: the flashbang fade's own (the shell's flash white is
//!   drawn), the damage type icons.
//! - Ours where the spec leaves it open: armour's lit rows truncated
//!   (24 x armour / 100); a hit from within 50 units needs the attacker's
//!   position (a network client, which has no attacker, lights by the
//!   direction alone); undirected damage (a fall) lights no arc; health
//!   and armour hidden while dead or spectating (the whole HUD then).

use std::collections::BTreeMap;

use bevy::{asset::RenderAssetUsages, prelude::*};
use chimera_client::client::{
    hud::{CrosshairColor, GAME_HUD_Z, GameDraws, HudRoot},
    hud_text::{GlyphParams, HudGlyphMaterial},
    weapon_menu::{WeaponMenu, menu_input},
    weapon_icons::{WeaponIcon, WeaponIcons},
};
use chimera_core::console::ConsoleAppExt;
use chimera_core::core::{DamageTaken, Dead, Health, Intent, LocalPlayer, MovementState, SimClock, Spectating, Velocity};
use chimera_sim::{
    mode::{Match, Phase},
    weapon::{AltModes, Armor, Inventory, Magazine, Weapon, Zoomed, grenade::Throwable},
};
use chimera_world::map::hud::UiImage;

use cs16::game::{GAME, INFO, METERS_PER_UNIT};

/// What this HUD draws instead of the shell's: the crosshair, health,
/// armour and ammo (the hit marker and kill feed stay the shell's and the
/// app's).
pub const DRAWS: GameDraws = GameDraws { crosshair: true, status: true, hit_marker: false, killfeed: false };

/// hud.md's constants.
pub mod consts {
    /// Every number and icon's tint.
    pub const HUD_COLOUR: [u8; 3] = [255, 160, 0];
    /// Health and its cross at `WARNING_HEALTH` or less.
    pub const WARNING_COLOUR: [u8; 3] = [250, 0, 0];
    pub const WARNING_HEALTH: u32 = 25;
    /// Health at or below this is drawn at full alpha.
    pub const BRIGHT_HEALTH: u32 = 15;
    /// A number at rest.
    pub const DIM: f32 = 100.0;
    /// A fade's start (health, armour) and how fast fades drain (a second).
    pub const FADE: f32 = 100.0;
    pub const DRAIN: f32 = 20.0;
    /// Alpha added to the dim alpha at a fade's start.
    pub const FLARE: f32 = 128.0;
    /// The ammunition's fade on each update.
    pub const AMMO_FLARE: f32 = 200.0;
    /// The `number_0` cell at 640.
    pub const FONT_HEIGHT: i32 = 25;
    pub const DIGIT_WIDTH: i32 = 20;
    /// The cross's and suit icons' size when their pictures aren't here.
    pub const ICON: i32 = 24;
    /// The kill feed's icon tint.
    pub const KILL_ICON_COLOUR: [u8; 3] = [255, 80, 0];
    /// Pain arcs: drawn above this strength, which drains this much a
    /// second; within this many units of the source, all four.
    pub const PAIN_SHOWN: f32 = 0.4;
    pub const PAIN_DRAIN: f32 = 2.0;
    pub const PAIN_NEAR: f32 = 50.0;
    pub const PAIN_SIDE: f32 = 0.3;
    /// The crosshair's defaults: `cl_crosshair_color` "50 250 50",
    /// `cl_crosshair_translucent` 1, `cl_crosshair_size` auto.
    pub const CROSSHAIR_COLOUR: [u8; 3] = [50, 250, 50];
    /// The gap's most after shots (a base above it stays), and what a
    /// drawn frame closes it by: a share of it and a flat pixel part.
    pub const CROSSHAIR_MOST_GAP: f32 = 15.0;
    pub const CROSSHAIR_CLOSE_SHARE: f32 = 0.013;
    pub const CROSSHAIR_CLOSE_FLAT: f32 = 0.1;
    /// The crosshair's alpha: at the start, what a shot takes off (down to
    /// the least) and what a frame gives back (up to 255).
    pub const CROSSHAIR_ALPHA: f32 = 200.0;
    pub const CROSSHAIR_SHOT_ALPHA: f32 = 40.0;
    pub const CROSSHAIR_LEAST_ALPHA: f32 = 120.0;
    pub const CROSSHAIR_REGAIN_ALPHA: f32 = 2.0;
    /// The scoped zoom steps (this field of view or less) hide it.
    pub const CROSSHAIR_HIDDEN_FOV: f32 = 40.0;
    /// The weapon menu: the top row's y, the space between cells, the
    /// gap under the slot numbers and right of the radar, the dim, empty
    /// and picture alphas, and the empty colour.
    pub const MENU_TOP: i32 = 10;
    pub const MENU_GAP: i32 = 5;
    pub const MENU_UNDER: i32 = 10;
    pub const EMPTY_COLOUR: [u8; 3] = [255, 16, 16];
    pub const MENU_BOX_ALPHA: u8 = 128;
    pub const MENU_BOX_EMPTY_ALPHA: u8 = 96;
    pub const MENU_PICTURE_ALPHA: u8 = 192;
    pub const MENU_EMPTY_ALPHA: u8 = 128;
    /// The ammunition bar's filled part's colour (the rest is the HUD
    /// colour at `MENU_BOX_ALPHA`).
    pub const MENU_BAR_COLOUR: [u8; 3] = [0, 160, 0];
    /// Slots the menu always shows.
    pub const MENU_SLOTS: usize = 5;
    /// The round timer flashes the reddish colour at this many seconds
    /// or less.
    pub const TIMER_WARNING: u32 = 20;
    /// A round with more than a day left has no limit worth showing (the
    /// sandbox's).
    pub const TIMER_MOST: u32 = 24 * 3600;
}

use consts::*;

/// The ammunition row's content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ammo {
    /// No ammunition type (the knife): no row.
    #[default]
    None,
    /// A magazine's clip and reserve.
    Magazine { clip: u32, reserve: u32 },
    /// What is carried (the grenades).
    Count(u32),
}

/// The crosshair's size setting (`cl_crosshair_size`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrossSize {
    #[default]
    Auto,
    Small,
    Medium,
    Large,
}

impl CrossSize {
    /// "auto", "small", "medium", "large" or 0-3.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "auto" | "0" => Self::Auto,
            "small" | "1" => Self::Small,
            "medium" | "2" => Self::Medium,
            "large" | "3" => Self::Large,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    /// The scale base on a screen `w` wide (hud.md, "Crosshair").
    pub fn base(self, w: i32) -> i32 {
        match self {
            Self::Auto if w <= 640 => w,
            Self::Auto if w < 1024 => 800,
            Self::Auto | Self::Large => 640,
            Self::Small => 1024,
            Self::Medium => 800,
        }
    }
}

/// The crosshair's console variables (`cl_crosshair_color`, `_translucent`
/// and `_size`; the shell's `cl_dynamiccrosshair` is read too).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrosshairLook {
    pub colour: [u8; 3],
    /// 1: added onto the frame; 0: blended.
    pub translucent: u8,
    pub size: CrossSize,
}

impl Default for CrosshairLook {
    fn default() -> Self {
        Self { colour: CROSSHAIR_COLOUR, translucent: 1, size: CrossSize::Auto }
    }
}

/// `cl_crosshair_color`'s value: three integers, each clamped to 0-255.
pub fn parse_colour(text: &str) -> Option<[u8; 3]> {
    let v: Vec<i64> = text.split_whitespace().map(|n| n.parse().ok()).collect::<Option<_>>()?;
    let [r, g, b] = v[..] else { return None };
    Some([r, g, b].map(|c| c.clamp(0, 255) as u8))
}

/// Whether the weapon's x1.4 applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boost {
    No,
    /// Always (the M4A1's: its test looks at a USP's silencer).
    Always,
    /// In the weapon's first mode (semi-automatic, automatic, unsilenced).
    FirstMode,
}

/// A weapon's crosshair (hud.md, "Per-weapon gap"): the base gap `base`
/// and the step `step` a shot opens it by, and its movement rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub base: i32,
    pub step: f32,
    /// Doubles in the air.
    pub air: bool,
    /// Halves ducked.
    pub duck: bool,
    /// Widens by 1.5 at this speed (units a second) or more, on the ground.
    pub speed: Option<f32>,
    pub boost: Boost,
}

impl Rule {
    const fn new(base: i32, step: f32) -> Self {
        Self { base, step, air: false, duck: false, speed: None, boost: Boost::No }
    }
    const fn moving(mut self, air: bool, duck: bool, speed: Option<f32>) -> Self {
        self.air = air;
        self.duck = duck;
        self.speed = speed;
        self
    }
    const fn boosted(mut self, boost: Boost) -> Self {
        self.boost = boost;
        self
    }
    fn has_movement(&self) -> bool {
        self.air || self.duck || self.speed.is_some() || self.boost != Boost::No
    }
}

/// The crosshair rule of weapon `id` (`cs16:weapon_ak47`); none: no
/// crosshair at all (the four sniper rifles). A weapon outside the table:
/// base 4, step 3, no movement rule.
pub fn rule_of(id: &str) -> Option<Rule> {
    let name = id.rsplit(':').next().unwrap_or(id);
    let name = name.strip_prefix("weapon_").unwrap_or(name);
    let pistol = Rule::new(8, 3.0).moving(true, true, Some(0.0));
    let rifle = |base, step, speed| Rule::new(base, step).moving(true, false, Some(speed));
    Some(match name {
        "scout" | "awp" | "g3sg1" | "sg550" => return None,
        "p228" | "deagle" | "fiveseven" => pistol,
        "glock18" | "usp" => pistol.boosted(Boost::FirstMode),
        "elite" => Rule::new(4, 3.0),
        "m3" => Rule::new(8, 6.0),
        "xm1014" => Rule::new(9, 4.0),
        "mac10" => Rule::new(9, 3.0).moving(true, false, None),
        "tmp" => Rule::new(7, 3.0).moving(true, false, None),
        "mp5navy" | "ump45" => Rule::new(6, 3.0).moving(true, false, None),
        "p90" => rifle(7, 3.0, 170.0),
        "galil" => rifle(4, 3.0, 140.0),
        "famas" => rifle(4, 3.0, 140.0).boosted(Boost::FirstMode),
        "ak47" => rifle(4, 4.0, 140.0),
        "m4a1" => rifle(4, 3.0, 140.0).boosted(Boost::Always),
        "sg552" => rifle(5, 3.0, 140.0),
        "aug" => rifle(3, 3.0, 140.0),
        "m249" => rifle(6, 3.0, 140.0),
        "knife" => Rule::new(7, 3.0),
        "hegrenade" | "flashbang" | "smokegrenade" => Rule::new(8, 3.0),
        "c4" => Rule::new(6, 3.0),
        _ => Rule::new(4, 3.0),
    })
}

/// The local player's movement as the crosshair reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    pub on_ground: bool,
    pub ducked: bool,
    /// The whole velocity's length, units a second.
    pub speed: f32,
}

impl Default for Stance {
    fn default() -> Self {
        Self { on_ground: true, ducked: false, speed: 0.0 }
    }
}

/// This frame's base gap `d` (hud.md, "This frame's base"): the rule's
/// base, then (`dynamic`: `cl_dynamiccrosshair`) the air, duck and speed
/// rules and the x1.4, each product truncated; `mode`: the weapon's
/// `AltModes` mode.
pub fn base_gap(rule: &Rule, stance: &Stance, mode: u8, dynamic: bool) -> i32 {
    let mut d = rule.base;
    if !dynamic || !rule.has_movement() {
        return d;
    }
    if rule.air && !stance.on_ground {
        d *= 2;
    } else if stance.ducked && rule.duck {
        d /= 2;
    } else if rule.speed.is_some_and(|v| stance.speed >= v) {
        d = d * 3 / 2;
    }
    if rule.boost == Boost::Always || (rule.boost == Boost::FirstMode && mode == 0) {
        d = d * 14 / 10;
    }
    d
}

/// The crosshair this frame: the base gap and step of the held weapon and
/// the variables it is drawn by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crosshair {
    pub base: i32,
    pub step: f32,
    pub look: CrosshairLook,
}

/// What the HUD shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Health (0-100, rounded up); none: no local player, dead or
    /// spectating (nothing but the pain arcs drawn).
    pub health: Option<u32>,
    /// Armour (0-100) and whether a helmet is worn.
    pub armour: (u32, bool),
    /// The weapon in hand's id.
    pub weapon_id: String,
    pub ammo: Ammo,
    /// The crosshair; none: not drawn (dead, a sniper rifle, a scope up,
    /// no player).
    pub crosshair: Option<Crosshair>,
    /// The weapon selection while it is open.
    pub menu: Option<Menu>,
    /// Whole seconds left of the round, while it runs.
    pub timer: Option<u32>,
}

/// One weapon in the selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuWeapon {
    pub id: String,
    /// No ammunition left (clip and reserve; a grenade's count).
    pub empty: bool,
    /// The reserve and the most the ammunition type carries (0: none, no
    /// bar).
    pub reserve: u32,
    pub most: u32,
}

/// The weapon selection: the weapons by slot (0-based) and the
/// highlighted one's slot and place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub slots: Vec<Vec<MenuWeapon>>,
    pub chosen: Option<(usize, usize)>,
}

/// The numbers' fades (fade units), the crosshair's gap state and the
/// timer's flash, and what the numbers last showed.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Fades {
    pub health: f32,
    pub armour: f32,
    pub ammo: f32,
    /// The crosshair's gap now, pixels at the 640 scale (hud.md's `g`),
    /// and its alpha.
    pub gap: f32,
    pub alpha: f32,
    /// Seconds into the timer's flash phase and whether it shows the
    /// reddish colour (the first phase after the start does).
    pub flash: f32,
    pub reddish: bool,
    last: Option<(Option<u32>, (u32, bool), String, Ammo)>,
}

impl Default for Fades {
    fn default() -> Self {
        Self { health: 0.0, armour: 0.0, ammo: 0.0, gap: 0.0, alpha: CROSSHAIR_ALPHA, flash: 0.0, reddish: true, last: None }
    }
}

impl Fades {
    /// Drain the fades by `dt` seconds, step the crosshair's gap and the
    /// timer's flash, then restart those fades whose value changed in `f`
    /// (the first frame seen restarts none). The gap and its alpha move
    /// once per call (per drawn frame, as hud.md has it), not per second.
    pub fn step(&mut self, f: &Frame, dt: f32) {
        for v in [&mut self.health, &mut self.armour, &mut self.ammo] {
            *v = (*v - DRAIN * dt).max(0.0);
        }
        // A shot: this weapon's clip fell since the last frame.
        let fired = matches!(
            (&self.last, f.ammo),
            (Some((_, _, w, Ammo::Magazine { clip: before, .. })), Ammo::Magazine { clip, .. }) if *w == f.weapon_id && clip < *before
        );
        if let Some(c) = &f.crosshair {
            if fired {
                self.gap = (self.gap + c.step).min(CROSSHAIR_MOST_GAP);
                self.alpha = (self.alpha - CROSSHAIR_SHOT_ALPHA).max(CROSSHAIR_LEAST_ALPHA);
            } else {
                self.gap -= CROSSHAIR_CLOSE_SHARE * self.gap + CROSSHAIR_CLOSE_FLAT;
                self.alpha += CROSSHAIR_REGAIN_ALPHA;
            }
            self.gap = self.gap.max(c.base as f32);
            self.alpha = self.alpha.min(255.0);
        }
        if let Some(t) = f.timer.filter(|t| *t <= TIMER_WARNING) {
            self.flash += dt;
            if self.flash > t as f32 / 40.0 + 0.1 {
                self.flash = 0.0;
                self.reddish = !self.reddish;
            }
        }
        let now = (f.health, f.armour, f.weapon_id.clone(), f.ammo);
        if let Some((health, armour, weapon, ammo)) = &self.last {
            if *health != f.health {
                self.health = FADE;
            }
            if armour.0 != f.armour.0 {
                self.armour = FADE;
            }
            if *weapon != f.weapon_id || *ammo != f.ammo {
                self.ammo = AMMO_FLARE;
            }
        }
        self.last = Some(now);
    }
}

/// Health's and armour's alpha at fade `fade` (hud.md, "Health").
pub fn number_alpha(fade: f32) -> u8 {
    if fade > 0.0 { (DIM + fade / FADE * FLARE).min(255.0) as u8 } else { DIM as u8 }
}

/// Health's alpha: `number_alpha`, full at 15 or less.
pub fn health_alpha(health: u32, fade: f32) -> u8 {
    if health <= BRIGHT_HEALTH { 255 } else { number_alpha(fade) }
}

/// The ammunition's alpha: the larger of the dim alpha and the fade.
pub fn ammo_alpha(fade: f32) -> u8 {
    fade.max(DIM).min(255.0) as u8
}

/// Health's colour.
pub fn health_colour(health: u32) -> [u8; 3] {
    if health <= WARNING_HEALTH { WARNING_COLOUR } else { HUD_COLOUR }
}

/// One thing drawn, in screen pixels (origin top left).
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// Picture `pic` (`HudPictures`' key) at (`x`, `y`), `src` its part
    /// drawn (`[x, y, w, h]` in its texels; the whole when none), tinted
    /// and added.
    Sprite { pic: String, x: i32, y: i32, src: Option<[u32; 4]>, colour: [u8; 3], alpha: u8 },
    /// A filled box `[x, y, w, h]`, blended.
    Fill { rect: [i32; 4], colour: [u8; 3], alpha: u8 },
    /// A filled box `[x, y, w, h]`, added onto the frame (the crosshair's
    /// lines).
    Add { rect: [i32; 4], colour: [u8; 3], alpha: u8 },
}

/// The key of weapon `id`'s ammunition icon in `HudPictures`.
pub fn ammo_key(id: &str) -> String {
    format!("ammo:{id}")
}

/// The keys of weapon `id`'s selection pictures (the plain, the lit).
pub fn menu_key(id: &str) -> String {
    format!("menu:{id}")
}

pub fn menu_lit_key(id: &str) -> String {
    format!("menu_s:{id}")
}

/// The key of pain arc `k` (0 front, 1 right, 2 behind, 3 left).
pub fn pain_key(k: usize) -> String {
    format!("pain_{k}")
}

/// The digits of `n` in three places from `x` (hud.md, "Numbers"): no
/// leading zeros, zero as one `0` in the last place, longer numbers
/// growing to the right; `dw`: a digit's width.
pub fn digits_by(n: u32, x: i32, dw: i32) -> Vec<(u32, i32)> {
    let s = n.to_string();
    let first = x + (3 - s.len() as i32).max(0) * dw;
    s.bytes().enumerate().map(|(k, b)| ((b - b'0') as u32, first + k as i32 * dw)).collect()
}

/// `digits_by` at the 640 layout's digit width.
pub fn digits(n: u32, x: i32) -> Vec<(u32, i32)> {
    digits_by(n, x, DIGIT_WIDTH)
}

/// The number cell's size of the layout (`number_0`'s picture; the 640
/// layout's when it isn't here): (digit width, font height).
#[derive(Clone, Copy)]
struct Cell {
    dw: i32,
    fh: i32,
}

fn number(out: &mut Vec<Draw>, n: u32, x: i32, y: i32, cell: Cell, colour: [u8; 3], alpha: u8) -> i32 {
    for (d, dx) in digits_by(n, x, cell.dw) {
        out.push(Draw::Sprite { pic: format!("number_{d}"), x: dx, y, src: None, colour, alpha });
    }
    x + (n.to_string().len() as i32).max(3) * cell.dw
}

/// The crosshair's four lines (hud.md, "Crosshair"): gap state `g` (at the
/// 640 scale), this frame's base `d`, alpha `a`, on a `w` x `h` screen;
/// left, right, up, down, one pixel thick. The length is `(g - d) / 2 + 5`
/// truncated; with the scale base `S` of `cl_crosshair_size` other than
/// `w`, the gap and length scale by `w / S`. Added onto the frame when
/// translucent, else blended.
pub fn crosshair_lines(c: &Crosshair, g: f32, a: f32, w: i32, h: i32) -> Vec<Draw> {
    let length = ((g - c.base as f32) * 0.5 + 5.0) as i32;
    let s = c.look.size.base(w);
    let (gap, len) = if s == w { (g, length) } else { (w as f32 * g / s as f32, (w as f32 * length as f32 / s as f32) as i32) };
    let (cx, cy) = ((w / 2) as f32, (h / 2) as f32);
    let (cx_i, cy_i) = (w / 2, h / 2);
    let rects = [
        [(cx - gap - len as f32 + 1.0) as i32, cy_i, len, 1],
        [(cx + gap) as i32, cy_i, len, 1],
        [cx_i, (cy - gap - len as f32 + 1.0) as i32, 1, len],
        [cx_i, (cy + gap) as i32, 1, len],
    ];
    let alpha = a.clamp(0.0, 255.0) as u8;
    rects
        .into_iter()
        .map(|rect| if c.look.translucent == 1 { Draw::Add { rect, colour: c.look.colour, alpha } } else { Draw::Fill { rect, colour: c.look.colour, alpha } })
        .collect()
}

/// The weapon selection laid out (hud.md, "Weapon menu"): the slot
/// numbers along the top from the radar's right, a box per weapon under a
/// closed slot, the open slot's pictures under its number with an
/// ammunition bar each; `w`: the screen's width (the layout).
fn menu_draws(out: &mut Vec<Draw>, m: &Menu, w: i32, size: &impl Fn(&str) -> Option<UVec2>) {
    let wide = w >= 640;
    let bucket = |slot: usize| format!("bucket{}", (slot + 1) % 10);
    let cell = size("bucket1").unwrap_or(UVec2::splat(if wide { 20 } else { 12 }));
    let radar = size("radar").map_or(if wide { 128 } else { 64 }, |s| s.x as i32);
    let fallback = if wide { UVec2::new(170, 45) } else { UVec2::new(80, 20) };
    let (bar_w, bar_h) = if wide { (20, 4) } else { (10, 2) };
    let top = cell.y as i32 + MENU_UNDER;
    let mut x = radar + MENU_UNDER;
    for slot in 0..m.slots.len().max(MENU_SLOTS) {
        let weapons = m.slots.get(slot).map(Vec::as_slice).unwrap_or(&[]);
        let open = m.chosen.is_some_and(|(s, _)| s == slot);
        let first = weapons.first().map_or(fallback, |w| size(&menu_key(&w.id)).unwrap_or(fallback));
        out.push(Draw::Sprite { pic: bucket(slot), x, y: MENU_TOP, src: None, colour: HUD_COLOUR, alpha: 255 });
        for (k, weapon) in weapons.iter().enumerate() {
            if open {
                let y = top + k as i32 * (first.y as i32 + MENU_GAP);
                let (colour, alpha) = if weapon.empty { (EMPTY_COLOUR, MENU_EMPTY_ALPHA) } else { (HUD_COLOUR, MENU_PICTURE_ALPHA) };
                if m.chosen == Some((slot, k)) {
                    out.push(Draw::Sprite { pic: menu_lit_key(&weapon.id), x, y, src: None, colour, alpha });
                    out.push(Draw::Sprite { pic: "selection".into(), x, y, src: None, colour, alpha });
                } else {
                    out.push(Draw::Sprite { pic: menu_key(&weapon.id), x, y, src: None, colour, alpha });
                }
                if weapon.reserve > 0 && weapon.most > 0 {
                    let f = (weapon.reserve as f32 / weapon.most as f32).clamp(0.0, 1.0);
                    let filled = ((f * bar_w as f32) as i32).max(1);
                    let bx = x + bar_w / 2;
                    out.push(Draw::Add { rect: [bx, y, filled, bar_h], colour: MENU_BAR_COLOUR, alpha: 255 });
                    if filled < bar_w {
                        out.push(Draw::Add { rect: [bx + filled, y, bar_w - filled, bar_h], colour: HUD_COLOUR, alpha: MENU_BOX_ALPHA });
                    }
                }
            } else {
                let y = top + k as i32 * (cell.y as i32 + MENU_GAP);
                let (colour, alpha) = if weapon.empty { (EMPTY_COLOUR, MENU_BOX_EMPTY_ALPHA) } else { (HUD_COLOUR, MENU_BOX_ALPHA) };
                out.push(Draw::Add { rect: [x, y, cell.x as i32, cell.y as i32], colour, alpha });
            }
        }
        x += if open { first.x as i32 } else { cell.x as i32 } + MENU_GAP;
    }
}

/// The round timer (hud.md, "Round timer"): the stopwatch with its left
/// edge on the screen's centre, the minutes in two places, a colon of two
/// 2 x 2 squares and the seconds in two, all at the dim alpha, in the
/// reddish colour in the warning's flash phase. Whole seconds `secs`.
fn timer_draws(out: &mut Vec<Draw>, secs: u32, reddish: bool, w: i32, h: i32, cell: Cell, size: &impl Fn(&str) -> Option<UVec2>) {
    let colour = if secs <= TIMER_WARNING && reddish { EMPTY_COLOUR } else { HUD_COLOUR };
    let alpha = DIM as u8;
    let sw = size("stopwatch").map_or(ICON, |s| s.x as i32);
    let y = (h as f32 - 1.5 * cell.fh as f32) as i32;
    out.push(Draw::Sprite { pic: "stopwatch".into(), x: w / 2, y, src: None, colour, alpha });
    let from = w / 2 + sw / 4;
    let minutes = (secs / 60).to_string();
    let first = from + (2 - minutes.len() as i32) * cell.dw;
    for (k, b) in minutes.bytes().enumerate() {
        out.push(Draw::Sprite { pic: format!("number_{}", b - b'0'), x: first + k as i32 * cell.dw, y, src: None, colour, alpha });
    }
    let end = from + 2 * cell.dw;
    for dy in [cell.fh / 4, cell.fh - cell.fh / 4] {
        out.push(Draw::Add { rect: [end + sw / 4, y + dy, 2, 2], colour, alpha });
    }
    let mut x = end + sw / 2;
    for d in [secs % 60 / 10, secs % 10] {
        out.push(Draw::Sprite { pic: format!("number_{d}"), x, y, src: None, colour, alpha });
        x += cell.dw;
    }
}

/// The frame laid out on a `w` x `h` screen (module doc; hud.md's
/// formulas), `size`: a picture's size if it is here; `pain`: the arcs'
/// strengths.
pub fn draws(f: &Frame, fades: &Fades, pain: &[f32; 4], w: i32, h: i32, size: impl Fn(&str) -> Option<UVec2>) -> Vec<Draw> {
    let mut out = Vec::new();
    let cell = size("number_0").map_or(Cell { dw: DIGIT_WIDTH, fh: FONT_HEIGHT }, |s| Cell { dw: s.x as i32, fh: s.y as i32 });
    let health_now = f.health.unwrap_or(100);
    // The crosshair, then the selection over the view.
    if f.health.is_some()
        && let Some(c) = &f.crosshair
    {
        out.extend(crosshair_lines(c, fades.gap, fades.alpha, w, h));
    }
    if let Some(m) = &f.menu {
        menu_draws(&mut out, m, w, &size);
    }
    // The pain arcs (hud.md, "Damage direction").
    for (k, s) in pain.iter().enumerate() {
        if *s <= PAIN_SHOWN {
            continue;
        }
        let key = pain_key(k);
        let Some(sz) = size(&key) else { continue };
        let (fw, fh) = (sz.x as i32, sz.y as i32);
        let (x, y) = match k {
            0 => (w / 2 - fw / 2, h / 2 - 3 * fh),
            1 => (w / 2 + 2 * fw, h / 2 - fh / 2),
            2 => (w / 2 - fw / 2, h / 2 + 2 * fh),
            _ => (w / 2 - 3 * fw, h / 2 - fh / 2),
        };
        let alpha = (s.max(0.5) * 255.0).min(255.0) as u8;
        out.push(Draw::Sprite { pic: key, x, y, src: None, colour: health_colour(health_now), alpha });
    }
    let Some(health) = f.health else { return out };
    let y0 = h - cell.fh - cell.fh / 2;
    let row = y0 + cell.fh / 5;
    // The round timer.
    if let Some(secs) = f.timer {
        timer_draws(&mut out, secs, fades.reddish, w, h, cell, &size);
    }
    // Health.
    let cross = size("cross").map_or(ICON, |s| s.x as i32);
    let colour = health_colour(health);
    let alpha = health_alpha(health, fades.health);
    out.push(Draw::Sprite { pic: "cross".into(), x: cross / 2, y: y0, src: None, colour, alpha });
    let end = number(&mut out, health, cross + cell.dw / 2, row, cell, colour, alpha);
    out.push(Draw::Fill { rect: [end + cell.dw / 2, row, cell.dw / 10, cell.fh], colour: HUD_COLOUR, alpha });
    // Armour.
    let (armour, helmet) = f.armour;
    let (empty, full) = if helmet { ("suithelmet_empty", "suithelmet_full") } else { ("suit_empty", "suit_full") };
    let icon = size(empty).map_or(UVec2::splat(ICON as u32), |s| s);
    let (ix, iy) = (w / 5, y0 - icon.y as i32 / 6);
    let alpha = number_alpha(fades.armour);
    out.push(Draw::Sprite { pic: empty.into(), x: ix, y: iy, src: None, colour: HUD_COLOUR, alpha });
    let lit = icon.y * armour.min(100) / 100;
    if lit > 0 {
        let top = icon.y - lit;
        out.push(Draw::Sprite { pic: full.into(), x: ix, y: iy + top as i32, src: Some([0, top, icon.x, lit]), colour: HUD_COLOUR, alpha });
    }
    number(&mut out, armour, ix + icon.x as i32, row, cell, HUD_COLOUR, alpha);
    // Ammunition.
    let key = ammo_key(&f.weapon_id);
    let icon = size(&key).map_or(UVec2::splat(ICON as u32), |s| s);
    let alpha = ammo_alpha(fades.ammo);
    let icon_x = match f.ammo {
        Ammo::None => None,
        Ammo::Magazine { clip, reserve } => {
            let x = w - 8 * cell.dw - icon.x as i32;
            let bar = number(&mut out, clip, x, row, cell, HUD_COLOUR, alpha) + cell.dw / 2;
            out.push(Draw::Fill { rect: [bar, row, cell.dw / 10, cell.fh], colour: HUD_COLOUR, alpha });
            Some(number(&mut out, reserve, bar + 2 + cell.dw / 2, row, cell, HUD_COLOUR, alpha))
        }
        Ammo::Count(n) => Some(number(&mut out, n, w - 4 * cell.dw - icon.x as i32, row, cell, HUD_COLOUR, alpha)),
    };
    if let Some(x) = icon_x {
        out.push(Draw::Sprite { pic: key, x, y: row - icon.y as i32 / 8, src: None, colour: HUD_COLOUR, alpha });
    }
    out
}

/// The arcs' strengths (front, right, behind, left) after a hit from
/// direction `from` (toward the source, any length; flat in the view's
/// frame by `yaw`), `near`: the source within 50 units.
pub fn hit_arcs(mut strengths: [f32; 4], from: Vec3, yaw: f32, near: bool) -> [f32; 4] {
    if near {
        return [1.0; 4];
    }
    let from = from.with_y(0.0).normalize_or_zero();
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let (f, r) = (from.dot(forward), from.dot(right));
    if f > PAIN_SIDE {
        strengths[0] = strengths[0].max(f);
    } else if f < -PAIN_SIDE {
        strengths[2] = strengths[2].max(-f);
    }
    if r > PAIN_SIDE {
        strengths[1] = strengths[1].max(r);
    } else if r < -PAIN_SIDE {
        strengths[3] = strengths[3].max(-r);
    }
    strengths
}

/// The HUD's last frame and the screen it is laid out on (physical
/// pixels and the window's scale factor).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Cs16HudState {
    pub frame: Frame,
    pub screen: (i32, i32, f32),
}

impl Default for Cs16HudState {
    fn default() -> Self {
        Self { frame: Frame::default(), screen: (640, 480, 1.0) }
    }
}

/// The pain arcs' strengths (front, right, behind, left).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Pain(pub [f32; 4]);

/// One layout's pictures by key (`ammo_key`, `menu_key`, `pain_key`, the
/// shared entries' names), with their sizes in texels.
pub type Pictures = BTreeMap<String, (Handle<Image>, UVec2)>;

/// The install's pictures as images, by layout (640, 320: `cs16::game::
/// hud::RESOLUTIONS`), and a white pixel for the crosshair's lines.
#[derive(Resource, Default)]
pub struct HudPictures(pub BTreeMap<u32, Pictures>, pub Option<Handle<Image>>);

impl HudPictures {
    /// The layout a `width` pixel wide screen draws: its own if its
    /// pictures are here, else the 640 ones.
    pub fn for_width(&self, width: i32) -> Option<&Pictures> {
        self.0.get(&cs16::game::hud::resolution_for(width.max(0) as u32)).or_else(|| self.0.get(&640))
    }
}

/// When `load_pictures` looks again (`Local`), by layout.
#[derive(Default)]
pub struct Tries {
    done: bool,
    next: f32,
    wait: f32,
}

const MOST_WAIT: f32 = 30.0;

fn image(images: &mut Assets<Image>, p: UiImage) -> (Handle<Image>, UVec2) {
    let size = UVec2::new(p.width, p.height);
    let image = Image::new(
        bevy::render::render_resource::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        p.rgba8,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    (images.add(image), size)
}

/// A grey additive picture as one blended in `tint`: each texel the tint
/// with its brightness as alpha (for the kill feed, which blends).
pub fn tinted(p: &UiImage, tint: [u8; 3]) -> UiImage {
    let rgba8 = p
        .rgba8
        .chunks_exact(4)
        .flat_map(|px| {
            let peak = px[0].max(px[1]).max(px[2]);
            [tint[0], tint[1], tint[2], peak]
        })
        .collect();
    UiImage { width: p.width, height: p.height, rgba8 }
}

/// CS 1.6's pictures from its mount (the install, or the asset store's
/// `players` set), tried at startup and again further apart (2 s doubling
/// up to 30 s) until their files are here, never fetching, each layout on
/// its own: the HUD's (`HudPictures`) and, from the 640 ones, the kill
/// icons and headshot mark, registered for every app's kill feed
/// (`WeaponIcons`).
pub fn load_pictures(
    mut icons: ResMut<WeaponIcons>,
    mut pictures: ResMut<HudPictures>,
    images: Option<ResMut<Assets<Image>>>,
    time: Option<Res<Time<Real>>>,
    mut tries: Local<BTreeMap<u32, Tries>>,
) {
    let now = time.map_or(0.0, |t| t.elapsed_secs());
    let Some(mut images) = images else { return };
    if pictures.1.is_none() {
        let white = UiImage { width: 1, height: 1, rgba8: vec![255; 4] };
        pictures.1 = Some(image(&mut images, white).0);
    }
    for resolution in cs16::game::hud::RESOLUTIONS {
        let t = tries.entry(resolution).or_default();
        if t.done || now < t.next {
            continue;
        }
        t.wait = (t.wait * 2.0).clamp(2.0, MOST_WAIT);
        t.next = now + t.wait;
        let Some(mount) = INFO.mount() else { return };
        let Some(read) = cs16::game::hud::pictures(&mount, resolution) else { continue };
        t.done = true;
        if resolution == 640 {
            let mut kill = |p: &UiImage| {
                let (image, size) = image(&mut images, tinted(p, KILL_ICON_COLOUR));
                WeaponIcon::Image { image, size: size.as_vec2() }
            };
            let kills: BTreeMap<String, WeaponIcon> = read.kill_icons.iter().map(|(id, p)| (id.clone(), kill(p))).collect();
            let skull = read.shared.get("d_skull").map(&mut kill);
            if let Some(head) = read.shared.get("d_headshot") {
                icons.register_mark(GAME, "headshot", kill(head));
            }
            info!("CS 1.6: {} kill icons, {} weapons' and {} shared HUD pictures", kills.len(), read.weapons.len(), read.shared.len());
            if !kills.is_empty() || skull.is_some() {
                icons.register(GAME, move |id| kills.get(id).cloned().or_else(|| skull.clone()));
            }
        }
        let mut out = BTreeMap::new();
        for (name, p) in read.shared {
            out.insert(name, image(&mut images, p));
        }
        for (id, w) in read.weapons {
            if let Some(p) = w.ammo {
                out.insert(ammo_key(&id), image(&mut images, p));
            }
            if let Some(p) = w.weapon {
                out.insert(menu_key(&id), image(&mut images, p));
            }
            if let Some(p) = w.weapon_s {
                out.insert(menu_lit_key(&id), image(&mut images, p));
            }
        }
        for (k, p) in read.pain.into_iter().take(4).enumerate() {
            out.insert(pain_key(k), image(&mut images, p));
        }
        pictures.0.insert(resolution, out);
    }
}

/// A sprite node of the pool (`sync`), by index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteSlot(pub usize);

/// A filled box of the pool (`sync`), by index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FillSlot(pub usize);

/// The root, hidden until CS 1.6's HUD is the gameplay's.
#[derive(Component)]
pub struct Cs16Root;

/// The root (its nodes are spawned by `sync` as the draws need them).
pub fn spawn_root(mut commands: Commands) {
    commands.spawn((
        Cs16Root,
        HudRoot(GAME),
        Visibility::Hidden,
        GlobalZIndex(GAME_HUD_Z),
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
    ));
}

/// `hud_fastswitch` (`cs16:hud_fastswitch`): 0 by default, as hud.md has
/// it (parity); 0: the slot keys open the weapon selection (hud.md,
/// "Weapon menu") and attack takes the weapon; 1: slot keys switch at once.
#[derive(Resource, Default)]
pub struct FastSwitch(pub u8);

/// Slot keys go through the shell's weapon selection with fast switch off
/// while this HUD has its pictures.
pub fn takes_slots(menu: Option<ResMut<WeaponMenu>>, fast: Res<FastSwitch>, pictures: Res<HudPictures>) {
    if let Some(mut menu) = menu {
        let takes = fast.0 == 0 && !pictures.0.is_empty();
        if menu.takes_slots != takes {
            menu.takes_slots = takes;
        }
    }
}

/// The weapon selection has no timeout (hud.md, "Closing"): while it is
/// open its clock is held at the start, so the shell never closes it; it
/// closes when the local player dies or spectates.
pub fn keep_open(menu: Option<ResMut<WeaponMenu>>, player: Option<Single<(Has<Dead>, Has<Spectating>), With<LocalPlayer>>>) {
    let Some(mut menu) = menu else { return };
    if !menu.choosing || menu.shown_for.is_none() {
        return;
    }
    if player.is_some_and(|p| p.0 || p.1) {
        menu.cancel();
    } else if menu.shown_for != Some(0.0) {
        menu.shown_for = Some(0.0);
    }
}

/// The weapons a player's frame reads.
type WeaponQuery<'w, 's> = Query<'w, 's, (&'static Weapon, Option<&'static Magazine>, Option<&'static Throwable>, Option<&'static AltModes>)>;

/// The weapon selection as the HUD shows it: the weapons held by slot and
/// the highlighted one's place.
fn menu_frame(menu: &WeaponMenu, inv: Option<&Inventory>, weapons: &WeaponQuery) -> Option<Menu> {
    if !menu.choosing {
        return None;
    }
    let mut slots: Vec<Vec<MenuWeapon>> = Vec::new();
    let mut chosen = None;
    for e in inv.map_or(&[][..], |i| &i.weapons[..]) {
        let Ok((w, mag, throw, _)) = weapons.get(*e) else { continue };
        let slot = w.slot as usize;
        if slots.len() <= slot {
            slots.resize(slot + 1, Vec::new());
        }
        let empty = match (mag, throw) {
            (Some(m), _) => m.clip + m.reserve == 0,
            (None, Some(t)) => t.count == 0,
            _ => false,
        };
        if menu.highlighted == Some(*e) {
            chosen = Some((slot, slots[slot].len()));
        }
        let (reserve, most) = mag.map_or((0, 0), |m| (m.reserve, m.reserve_max));
        slots[slot].push(MenuWeapon { id: w.id.to_string(), empty, reserve, most });
    }
    Some(Menu { slots, chosen })
}

/// The local player's state into the frame, the fades stepped, the
/// window's size read.
#[allow(clippy::type_complexity)]
pub fn collect(
    player: Option<
        Single<
            (&Health, Option<&Armor>, Option<&Inventory>, Has<Dead>, Has<Spectating>, Option<&Zoomed>, Option<&Velocity>, Option<&MovementState>),
            With<LocalPlayer>,
        >,
    >,
    weapons: WeaponQuery,
    windows: Query<&Window>,
    menu: Option<Res<WeaponMenu>>,
    (round, clock, crosshair, look): (Option<Res<Match>>, Option<Res<SimClock>>, Option<Res<CrosshairColor>>, Option<Res<CrosshairLook>>),
    time: Option<Res<Time>>,
    mut fades: ResMut<Fades>,
    mut state: ResMut<Cs16HudState>,
) {
    let screen = windows
        .iter()
        .next()
        .map_or(state.screen, |w| (w.physical_width() as i32, w.physical_height() as i32, w.scale_factor()));
    let f = match player {
        Some(p) => {
            let (health, armour, inv, dead, spectating, zoomed, velocity, movement) = *p;
            let held = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
            let ammo = match held {
                Some((_, Some(m), ..)) => Ammo::Magazine { clip: m.clip, reserve: m.reserve },
                Some((_, None, Some(t), _)) => Ammo::Count(t.count),
                _ => Ammo::None,
            };
            let alive = !dead && !spectating;
            let dynamic = crosshair.is_none_or(|c| c.dynamic != 0);
            let stance = Stance {
                on_ground: movement.is_none_or(|m| m.on_ground),
                ducked: movement.is_some_and(|m| m.crouching),
                speed: velocity.map_or(0.0, |v| v.0.length() / METERS_PER_UNIT),
            };
            let mode = held.and_then(|(.., a)| a).map_or(0, |a| a.current);
            let scoped = zoomed.is_some_and(|z| z.fov <= CROSSHAIR_HIDDEN_FOV);
            let crosshair = held
                .and_then(|(w, ..)| rule_of(w.id))
                .filter(|_| alive && !scoped)
                .map(|rule| Crosshair { base: base_gap(&rule, &stance, mode, dynamic), step: rule.step, look: look.map_or_else(CrosshairLook::default, |l| *l) });
            let timer = round.zip(clock).and_then(|(m, c)| {
                (m.phase == Phase::Live)
                    .then(|| m.round_ends.map(|e| (e - c.now).max(0.0) as u32))
                    .flatten()
                    .filter(|left| *left <= TIMER_MOST)
            });
            Frame {
                health: alive.then(|| (health.current * 100.0).ceil().max(0.0) as u32),
                armour: armour.map_or((0, false), |a| ((a.amount * 100.0).round().max(0.0) as u32, a.helmet)),
                weapon_id: held.map(|(w, ..)| w.id.to_string()).unwrap_or_default(),
                ammo,
                crosshair,
                menu: menu.and_then(|m| menu_frame(&m, inv, &weapons)).filter(|_| alive),
                timer,
            }
        }
        None => Frame::default(),
    };
    fades.step(&f, time.map_or(0.0, |t| t.delta_secs()));
    if state.frame != f || state.screen != screen {
        state.frame = f;
        state.screen = screen;
    }
}

/// The arcs' strengths drained, and raised by the hits on the local
/// player (`core::DamageTaken`; a network client's from the server).
pub fn pain(
    mut taken: MessageReader<DamageTaken>,
    me: Option<Single<(Entity, &Intent, &GlobalTransform), With<LocalPlayer>>>,
    places: Query<&GlobalTransform>,
    time: Option<Res<Time>>,
    mut pain: ResMut<Pain>,
) {
    let dt = time.map_or(0.0, |t| t.delta_secs());
    let mut s = pain.0.map(|v| (v - PAIN_DRAIN * dt).max(0.0));
    if let Some(me) = me {
        let (me, intent, at) = *me;
        for t in taken.read() {
            let d = &t.damage;
            if d.target != me || !t.directed {
                continue;
            }
            let source = d.attacker.and_then(|a| places.get(a).ok()).map(|g| g.translation());
            let near = source.is_some_and(|p| p.distance(at.translation()) <= PAIN_NEAR * METERS_PER_UNIT);
            let from = source.map_or(-d.dir, |p| p - at.translation());
            s = hit_arcs(s, from, intent.yaw, near);
        }
    } else {
        taken.clear();
    }
    if pain.0 != s {
        pain.0 = s;
    }
}

fn colour(c: [u8; 3], alpha: u8) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], alpha)
}

/// The draws into the pool's nodes (each a `HudGlyphMaterial`, additive,
/// or a filled box), at the window's pixels (physical / scale factor).
#[allow(clippy::type_complexity)]
pub fn sync(
    state: Res<Cs16HudState>,
    fades: Res<Fades>,
    pain: Res<Pain>,
    pictures: Res<HudPictures>,
    root: Option<Single<Entity, With<Cs16Root>>>,
    mut materials: ResMut<Assets<HudGlyphMaterial>>,
    mut sprites: Query<(&SpriteSlot, &MaterialNode<HudGlyphMaterial>, &mut Node, &mut Visibility), Without<FillSlot>>,
    mut fills: Query<(&FillSlot, &mut Node, &mut BackgroundColor, &mut Visibility), Without<SpriteSlot>>,
    mut commands: Commands,
) {
    let Some(root) = root else { return };
    let (w, h, sf) = state.screen;
    let sf = if sf > 0.0 { sf } else { 1.0 };
    let layout = pictures.for_width(w);
    let all = draws(&state.frame, &fades, &pain.0, w, h, |k| layout.and_then(|l| l.get(k)).map(|p| p.1));
    // Each sprite: its image, picture size, place, node size, the part
    // of the picture shown (UV rectangle, in texels) and colour.
    let mut want_sprites = Vec::new();
    let mut want_fills = Vec::new();
    for d in all {
        match d {
            Draw::Sprite { pic, x, y, src, colour: c, alpha } => {
                if let Some((image, size)) = layout.and_then(|l| l.get(&pic)) {
                    let [sx, sy, sw, sh] = src.unwrap_or([0, 0, size.x, size.y]);
                    want_sprites.push((image.clone(), *size, x, y, [sw, sh], [sx, sy, sw, sh], colour(c, alpha)));
                }
            }
            Draw::Add { rect: [x, y, rw, rh], colour: c, alpha } => {
                if let Some(white) = &pictures.1 {
                    want_sprites.push((white.clone(), UVec2::ONE, x, y, [rw as u32, rh as u32], [0, 0, 1, 1], colour(c, alpha)));
                }
            }
            Draw::Fill { rect, colour: c, alpha } => want_fills.push((rect, colour(c, alpha))),
        }
    }
    let px = |v: i32| Val::Px(v as f32 / sf);
    let mut have = 0;
    for (slot, material, mut node, mut vis) in &mut sprites {
        have = have.max(slot.0 + 1);
        let Some((image, size, x, y, [nw, nh], [sx, sy, sw, sh], c)) = want_sprites.get(slot.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let uv = Vec4::new(*sx as f32 / size.x as f32, *sy as f32 / size.y as f32, *sw as f32 / size.x as f32, *sh as f32 / size.y as f32);
        let params = GlyphParams { color: c.to_linear(), uv };
        if let Some(m) = materials.get(&material.0)
            && (m.params != params || m.coverage != *image)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.params = params;
            m.coverage = image.clone();
        }
        let (l, t, ww, hh) = (px(*x), px(*y), px(*nw as i32), px(*nh as i32));
        if node.left != l || node.top != t || node.width != ww || node.height != hh {
            node.left = l;
            node.top = t;
            node.width = ww;
            node.height = hh;
        }
        vis.set_if_neq(Visibility::Inherited);
    }
    for k in have..want_sprites.len() {
        let (image, ..) = &want_sprites[k];
        let m = materials.add(HudGlyphMaterial { params: GlyphParams::whole(Color::NONE), coverage: image.clone(), additive: true });
        let node = Node { position_type: PositionType::Absolute, ..default() };
        commands.entity(*root).with_child((SpriteSlot(k), MaterialNode(m), node, Visibility::Hidden));
    }
    let mut have = 0;
    for (slot, mut node, mut bg, mut vis) in &mut fills {
        have = have.max(slot.0 + 1);
        let Some(([x, y, ww, hh], c)) = want_fills.get(slot.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let (l, t, wv, hv) = (px(*x), px(*y), px(*ww), px(*hh));
        if node.left != l || node.top != t || node.width != wv || node.height != hv {
            node.left = l;
            node.top = t;
            node.width = wv;
            node.height = hv;
        }
        bg.set_if_neq(BackgroundColor(*c));
        vis.set_if_neq(Visibility::Inherited);
    }
    for k in have..want_fills.len() {
        let node = Node { position_type: PositionType::Absolute, ..default() };
        commands.entity(*root).with_child((FillSlot(k), node, BackgroundColor(Color::NONE), Visibility::Hidden));
    }
}

/// `cl_crosshair_color`, `_translucent` and `_size` (hud.md, "Console
/// variables"): a bad value prints its usage; translucent is then reset to
/// 1 and the size kept.
fn register_crosshair_cvars(app: &mut App) {
    let d = CrosshairLook::default();
    app.console_cvar(
        "cs16:cl_crosshair_color",
        "CS 1.6's crosshair colour: three integers 0-255, \"50 250 50\".",
        "50 250 50",
        |w| w.get_resource::<CrosshairLook>().map(|l| format!("{} {} {}", l.colour[0], l.colour[1], l.colour[2])),
        |w, v| {
            let c = parse_colour(v).ok_or("usage: cs16:cl_crosshair_color \"<red> <green> <blue>\" (0-255 each)")?;
            w.get_resource_or_init::<CrosshairLook>().colour = c;
            Ok(())
        },
    );
    app.console_cvar(
        "cs16:cl_crosshair_translucent",
        "1: the crosshair is added onto the frame; 0: blended at its alpha.",
        "1",
        |w| w.get_resource::<CrosshairLook>().map(|l| l.translucent.to_string()),
        |w, v| {
            let t = v.trim().parse::<u8>().ok().filter(|t| *t <= 1);
            w.get_resource_or_init::<CrosshairLook>().translucent = t.unwrap_or(1);
            t.map(|_| ()).ok_or_else(|| "usage: cs16:cl_crosshair_translucent 0 or 1 (reset to 1)".to_string())
        },
    );
    app.console_cvar(
        "cs16:cl_crosshair_size",
        "CS 1.6's crosshair size: auto, small, medium, large (or 0-3).",
        d.size.name(),
        |w| w.get_resource::<CrosshairLook>().map(|l| l.size.name().to_string()),
        |w, v| {
            let s = CrossSize::parse(v).ok_or("usage: cs16:cl_crosshair_size auto, small, medium or large (0-3); kept")?;
            w.get_resource_or_init::<CrosshairLook>().size = s;
            Ok(())
        },
    );
}

/// CS 1.6's HUD: its state, root and nodes, drawing while its HUD is the
/// gameplay's; its pictures and kill icons loaded once here.
pub struct Cs16HudPlugin;

impl Plugin for Cs16HudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().get_resource_or_init::<chimera_client::client::hud::HudDraws>().0.push((GAME, DRAWS));
        // Its kill feed is the app's: it shows none of its own.
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, |_| 0);
        app.init_resource::<FastSwitch>();
        chimera_core::console::resource_cvar::<FastSwitch, u8>(
            app,
            "cs16:hud_fastswitch",
            "0: a slot key opens the weapon selection, attack takes the weapon (CS 1.6's default); 1: slot keys switch at once.",
            |f| &mut f.0,
        );
        app.init_resource::<CrosshairLook>();
        register_crosshair_cvars(app);
        app.init_resource::<Cs16HudState>()
            .init_resource::<Fades>()
            .init_resource::<Pain>()
            .init_resource::<HudPictures>()
            .init_resource::<FastSwitch>()
            .init_resource::<WeaponIcons>()
            .add_message::<DamageTaken>()
            .add_systems(Startup, spawn_root)
            .add_systems(
                Update,
                (
                    // Before the shell's selection reads the slot keys.
                    (takes_slots, keep_open).before(menu_input).run_if(chimera_core::gameplay::hud_is(GAME)),
                    (collect, pain, sync).chain().run_if(chimera_core::gameplay::hud_is(GAME)),
                    load_pictures,
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes(k: &str) -> Option<UVec2> {
        match k {
            "cross" | "suit_empty" | "suit_full" | "suithelmet_empty" | "suithelmet_full" => Some(UVec2::splat(24)),
            k if k.starts_with("ammo:") => Some(UVec2::splat(24)),
            "pain_0" | "pain_2" => Some(UVec2::new(128, 48)),
            "pain_1" | "pain_3" => Some(UVec2::new(48, 128)),
            _ => None,
        }
    }

    fn sprites(d: &[Draw], prefix: &str) -> Vec<(String, i32, i32)> {
        d.iter()
            .filter_map(|d| match d {
                Draw::Sprite { pic, x, y, .. } if pic.starts_with(prefix) => Some((pic.clone(), *x, *y)),
                _ => None,
            })
            .collect()
    }

    fn at(health: u32) -> Frame {
        Frame { health: Some(health), ..default() }
    }

    /// hud.md's test cases 1 and 2.
    #[test]
    fn health_sits_by_the_formulas() {
        let d = draws(&at(100), &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "cross"), vec![("cross".into(), 12, 443)]);
        let nums: Vec<_> = sprites(&d, "number_").into_iter().take(3).collect();
        assert_eq!(nums, vec![("number_1".into(), 34, 448), ("number_0".into(), 54, 448), ("number_0".into(), 74, 448)]);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [104, 448, 2, 25], .. })));
        let d = draws(&at(7), &Fades::default(), &[0.0; 4], 640, 480, sizes);
        let seven = d.iter().find(|d| matches!(d, Draw::Sprite { pic, .. } if pic == "number_7")).unwrap();
        assert!(matches!(seven, Draw::Sprite { x: 74, colour: WARNING_COLOUR, alpha: 255, .. }));
    }

    /// hud.md's test cases 3 and 4, and the ammunition's (9).
    #[test]
    fn fades_follow_the_spec() {
        let mut f = Fades::default();
        f.step(&at(100), 0.0);
        f.step(&at(60), 0.0);
        f.step(&at(60), 2.5);
        assert_eq!(health_alpha(60, f.health), 164);
        f.step(&at(60), 3.5);
        assert_eq!(health_alpha(60, f.health), 100);
        let mut f = Fades::default();
        let ak = |clip| Frame { health: Some(100), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip, reserve: 90 }, ..default() };
        f.step(&ak(30), 0.0);
        f.step(&ak(29), 0.0);
        assert_eq!(ammo_alpha(f.ammo), 200);
        f.step(&ak(29), 2.0);
        assert_eq!(ammo_alpha(f.ammo), 160);
    }

    /// hud.md's test cases 5 and 6.
    #[test]
    fn armour_lights_its_lower_part() {
        let f = Frame { health: Some(100), armour: (50, false), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "suit_empty"), vec![("suit_empty".into(), 128, 439)]);
        let lit = d.iter().find_map(|d| match d {
            Draw::Sprite { pic, y, src, .. } if pic == "suit_full" => Some((*y, *src)),
            _ => None,
        });
        assert_eq!(lit, Some((451, Some([0, 12, 24, 12]))));
        let nums = sprites(&d, "number_");
        assert_eq!(nums[3..], [("number_5".to_string(), 172, 448), ("number_0".to_string(), 192, 448)]);
        let d = draws(&f, &Fades::default(), &[0.0; 4], 1920, 1080, sizes);
        assert_eq!(sprites(&d, "suit_empty")[0].1, 384);
    }

    /// hud.md's test cases 7 and 8.
    #[test]
    fn ammunition_sits_by_the_formulas() {
        let f = Frame { health: Some(100), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip: 30, reserve: 90 }, ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        let xs: Vec<i32> = sprites(&d, "number_").iter().skip(4).map(|s| s.1).collect();
        assert_eq!(xs, vec![476, 496, 558, 578]);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [526, 448, 2, 25], .. })));
        assert_eq!(sprites(&d, "ammo:"), vec![("ammo:cs16:weapon_ak47".into(), 598, 445)]);
        let f = Frame { health: Some(100), weapon_id: "cs16:weapon_hegrenade".into(), ammo: Ammo::Count(1), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, sizes);
        assert_eq!(sprites(&d, "number_").last().unwrap().1, 576);
        assert_eq!(sprites(&d, "ammo:")[0].1, 596);
        let knife = Frame { health: Some(100), weapon_id: "cs16:weapon_knife".into(), ..default() };
        assert!(sprites(&draws(&knife, &Fades::default(), &[0.0; 4], 640, 480, sizes), "ammo:").is_empty());
    }

    fn ak_crosshair(base: i32) -> Crosshair {
        Crosshair { base, step: 4.0, look: CrosshairLook::default() }
    }

    fn with(c: Crosshair, clip: u32) -> Frame {
        Frame { health: Some(100), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip, reserve: 90 }, crosshair: Some(c), ..default() }
    }

    fn stance(on_ground: bool, ducked: bool, speed: f32) -> Stance {
        Stance { on_ground, ducked, speed }
    }

    fn base(id: &str, s: Stance, mode: u8) -> i32 {
        base_gap(&rule_of(id).unwrap(), &s, mode, true)
    }

    /// hud.md's test cases: the base gap by weapon, stance and mode.
    #[test]
    fn the_base_gap_follows_the_weapon_rules() {
        let (still, run, air, duck) = (stance(true, false, 0.0), stance(true, false, 250.0), stance(false, false, 0.0), stance(true, true, 0.0));
        assert_eq!([base("cs16:weapon_ak47", still, 0), base("cs16:weapon_ak47", run, 0), base("cs16:weapon_ak47", air, 0)], [4, 6, 8]);
        // The M4A1: x1.4 with the silencer on or off (mode 0 or 1).
        assert_eq!([base("cs16:weapon_m4a1", still, 0), base("cs16:weapon_m4a1", still, 1)], [5, 5]);
        assert_eq!([base("cs16:weapon_m4a1", run, 0), base("cs16:weapon_m4a1", air, 0)], [8, 11]);
        // A pistol's speed rule starts at 0: standing already widens it.
        assert_eq!([base("cs16:weapon_deagle", still, 0), base("cs16:weapon_deagle", duck, 0), base("cs16:weapon_deagle", air, 0)], [12, 4, 16]);
        assert_eq!([base("cs16:weapon_glock18", still, 0), base("cs16:weapon_glock18", still, 1)], [16, 12], "semi-automatic, burst");
        assert_eq!([base("cs16:weapon_usp", still, 0), base("cs16:weapon_usp", still, 1)], [16, 12], "without, with the silencer");
        assert_eq!([base("cs16:weapon_famas", stance(true, false, 0.0), 0), base("cs16:weapon_famas", still, 1)], [5, 4]);
        assert_eq!([base("cs16:weapon_mp5navy", still, 0), base("cs16:weapon_mp5navy", air, 0)], [6, 12]);
        // The p90 widens from 170, the AK from 140; shotguns and the elite never move.
        assert_eq!([base("cs16:weapon_p90", stance(true, false, 160.0), 0), base("cs16:weapon_p90", stance(true, false, 170.0), 0)], [7, 10]);
        assert_eq!([base("cs16:weapon_m3", air, 0), base("cs16:weapon_elite", run, 0), base("cs16:weapon_knife", air, 0)], [8, 4, 7]);
        // Not in the table: 4, no rule.
        assert_eq!(base("cs16:weapon_unknown", air, 0), 4);
        // cl_dynamiccrosshair 0: movement doesn't widen it.
        assert_eq!(base_gap(&rule_of("cs16:weapon_ak47").unwrap(), &air, 0, false), 4);
        // The snipers have none at all.
        for id in ["scout", "awp", "g3sg1", "sg550"] {
            assert_eq!(rule_of(&format!("cs16:weapon_{id}")), None);
        }
        assert_eq!(rule_of("cs16:weapon_aug").map(|r| r.base), Some(3));
    }

    /// hud.md's test cases: shots open the gap by a step, a frame closes
    /// it 1.3 % + 0.1 pixel, per drawn frame whatever its length.
    #[test]
    fn the_gap_opens_by_a_step_per_shot_and_closes_per_frame() {
        let mut f = Fades::default();
        f.step(&with(ak_crosshair(4), 30), 0.016);
        // At rest: the base, the alpha 200 + 2 (it starts at 200).
        assert_eq!((f.gap, f.alpha), (4.0, 202.0));
        // One shot: 4 + 4 = 8, the alpha 40 down from where it was.
        let before = f.alpha;
        f.step(&with(ak_crosshair(4), 29), 0.0);
        assert_eq!((f.gap, f.alpha), (8.0, before - 40.0));
        // A frame without one: 8 - (0.104 + 0.1).
        f.step(&with(ak_crosshair(4), 29), 0.0);
        assert!((f.gap - 7.796).abs() < 1e-4, "{}", f.gap);
        // The frame's length changes nothing.
        let mut g = f.clone();
        f.step(&with(ak_crosshair(4), 29), 0.001);
        g.step(&with(ak_crosshair(4), 29), 1.0);
        assert_eq!(f.gap, g.gap);
        // From 15 back to the AK's 4 takes 51 frames.
        let mut f = Fades { gap: 15.0, ..default() };
        let mut frames = 0;
        while f.gap > 4.0 {
            f.step(&with(ak_crosshair(4), 30), 0.01);
            frames += 1;
        }
        assert_eq!((frames, f.gap), (51, 4.0));
        // Three shots in three frames of the M3 (step 6): up to 15.
        let m3 = |clip| Frame { weapon_id: "cs16:weapon_m3".into(), crosshair: Some(Crosshair { base: 8, step: 6.0, look: CrosshairLook::default() }), ..with(ak_crosshair(8), clip) };
        let mut f = Fades::default();
        f.step(&m3(8), 0.0);
        assert_eq!(f.gap, 8.0);
        f.step(&m3(7), 0.0);
        assert_eq!(f.gap, 14.0);
        f.step(&m3(6), 0.0);
        assert_eq!(f.gap, 15.0, "clamped");
        // A base above 15 stays; the grenades and knife (no clip) never open it.
        let mut f = Fades::default();
        f.step(&with(ak_crosshair(16), 30), 0.0);
        assert_eq!(f.gap, 16.0);
        let he = |n| Frame { health: Some(100), weapon_id: "cs16:weapon_hegrenade".into(), ammo: Ammo::Count(n), crosshair: Some(Crosshair { base: 8, step: 3.0, look: CrosshairLook::default() }), ..default() };
        let mut f = Fades::default();
        f.step(&he(2), 0.0);
        f.step(&he(1), 0.0);
        assert_eq!(f.gap, 8.0);
    }

    /// hud.md's test cases: the lines' places and lengths, the scale
    /// bases, translucent or blended.
    #[test]
    fn the_lines_follow_the_size_setting() {
        let rects = |l: Vec<Draw>| l.into_iter().map(|d| match d { Draw::Add { rect, .. } | Draw::Fill { rect, .. } => rect, _ => panic!() }).collect::<Vec<_>>();
        let c = ak_crosshair(4);
        // 640 x 480 at rest: left 312-316, right 324-328, top 232-236, bottom 244-248.
        assert_eq!(rects(crosshair_lines(&c, 4.0, 200.0, 640, 480)), vec![[312, 240, 5, 1], [324, 240, 5, 1], [320, 232, 1, 5], [320, 244, 1, 5]]);
        // One shot: gap 8, length (8 - 4) / 2 + 5 = 7.
        assert_eq!(rects(crosshair_lines(&c, 8.0, 160.0, 640, 480))[0], [306, 240, 7, 1]);
        // Auto at 1920 x 1080: scale 3, gap 12, lines 15.
        assert_eq!(rects(crosshair_lines(&c, 4.0, 200.0, 1920, 1080)), vec![[934, 540, 15, 1], [972, 540, 15, 1], [960, 514, 1, 15], [960, 552, 1, 15]]);
        // Auto at 800 x 600: as at 640 (scale 1).
        assert_eq!(rects(crosshair_lines(&c, 4.0, 200.0, 800, 600))[1], [404, 300, 5, 1]);
        // Rest lengths with auto: 5 up to 800 wide (the table's scale 800 gives 6 at 1023, where
        // the prose says 5: the table is followed), 8 at 1024, 10 at 1280, 15 at 1920.
        let long = |w| match crosshair_lines(&c, 4.0, 200.0, w, w * 3 / 4)[0] { Draw::Add { rect, .. } => rect[2], _ => 0 };
        assert_eq!([long(640), long(800), long(1023), long(1024), long(1280), long(1920)], [5, 5, 6, 8, 10, 15]);
        // Small at 640 x 480: gap 2.5, lines 3.
        let small = Crosshair { look: CrosshairLook { size: CrossSize::Small, ..default() }, ..c };
        assert_eq!(rects(crosshair_lines(&small, 4.0, 200.0, 640, 480))[..2], [[315, 240, 3, 1], [322, 240, 3, 1]]);
        // The colour, alpha and blending by the variables.
        let blended = Crosshair { look: CrosshairLook { translucent: 0, colour: [1, 2, 3], ..default() }, ..c };
        assert_eq!(crosshair_lines(&blended, 4.0, 200.0, 640, 480)[0], Draw::Fill { rect: [312, 240, 5, 1], colour: [1, 2, 3], alpha: 200 });
        assert!(matches!(crosshair_lines(&c, 4.0, 200.0, 640, 480)[0], Draw::Add { colour: CROSSHAIR_COLOUR, alpha: 200, .. }));
    }

    #[test]
    fn the_variables_parse_as_the_spec_says() {
        assert_eq!(parse_colour("50 250 50"), Some([50, 250, 50]));
        assert_eq!(parse_colour("300 -4 7"), Some([255, 0, 7]));
        assert_eq!(parse_colour("1 2"), None);
        assert_eq!(parse_colour("red"), None);
        assert_eq!(CrossSize::parse("Large"), Some(CrossSize::Large));
        assert_eq!(CrossSize::parse("2"), Some(CrossSize::Medium));
        assert_eq!(CrossSize::parse("4"), None);
        assert_eq!(CrosshairLook::default(), CrosshairLook { colour: [50, 250, 50], translucent: 1, size: CrossSize::Auto });
    }

    /// hud.md's test cases: the timer's flash phases.
    #[test]
    fn the_timer_flashes_reddish_at_twenty_seconds() {
        let at = |t| Frame { health: Some(100), timer: Some(t), ..default() };
        let mut f = Fades::default();
        assert!(f.reddish, "the first warning phase is the reddish one");
        f.step(&at(25), 5.0);
        assert!(f.reddish && f.flash == 0.0, "above 20 s it doesn't run");
        // 20 s left: the phase lasts 0.6 s.
        f.step(&at(20), 0.6);
        assert!(f.reddish);
        f.step(&at(20), 0.001);
        assert!(!f.reddish && f.flash == 0.0);
        // At 19 s: 0.575 s.
        f.step(&at(19), 0.575);
        assert!(!f.reddish);
        f.step(&at(19), 0.001);
        assert!(f.reddish);
    }

    /// hud.md's timer test cases, 640 x 480 and 320 x 240.
    #[test]
    fn the_timer_sits_by_the_formulas() {
        let stop = |k: &str| match k {
            "stopwatch" => Some(UVec2::splat(24)),
            "number_0" => Some(UVec2::new(20, 25)),
            _ => None,
        };
        let draws_at = |t, reddish, w, h, sz: &dyn Fn(&str) -> Option<UVec2>| {
            let mut out = Vec::new();
            let cell = sz("number_0").map_or(Cell { dw: 20, fh: 25 }, |s| Cell { dw: s.x as i32, fh: s.y as i32 });
            timer_draws(&mut out, t, reddish, w, h, cell, &sz);
            out
        };
        let d = draws_at(164, true, 640, 480, &stop);
        let at: Vec<_> = d.iter().map(|d| match d {
            Draw::Sprite { pic, x, y, .. } => (pic.clone(), *x, *y),
            Draw::Add { rect, .. } => ("sq".into(), rect[0], rect[1]),
            _ => panic!(),
        }).collect();
        assert_eq!(at, vec![
            ("stopwatch".into(), 320, 442), ("number_2".into(), 346, 442), ("sq".into(), 372, 448), ("sq".into(), 372, 461),
            ("number_4".into(), 378, 442), ("number_4".into(), 398, 442),
        ]);
        // Always dim; 25 s the HUD colour, 20 s reddish in its phase, the HUD colour in the other.
        assert!(d.iter().all(|d| matches!(d, Draw::Sprite { colour: HUD_COLOUR, alpha: 100, .. } | Draw::Add { colour: HUD_COLOUR, alpha: 100, .. })));
        assert!(draws_at(20, true, 640, 480, &stop).iter().all(|d| matches!(d, Draw::Sprite { colour: EMPTY_COLOUR, .. } | Draw::Add { colour: EMPTY_COLOUR, .. })));
        assert!(draws_at(20, false, 640, 480, &stop).iter().all(|d| matches!(d, Draw::Sprite { colour: HUD_COLOUR, .. } | Draw::Add { colour: HUD_COLOUR, .. })));
        assert!(draws_at(25, true, 640, 480, &stop).iter().all(|d| matches!(d, Draw::Sprite { colour: HUD_COLOUR, .. } | Draw::Add { colour: HUD_COLOUR, .. })));
        // 65 s: the seconds `0` and `5` at 378 and 398; the minutes' 1 in the second place.
        let d = draws_at(65, true, 640, 480, &stop);
        assert_eq!(sprites(&d, "number_").iter().map(|s| (s.0.clone(), s.1)).collect::<Vec<_>>(), vec![("number_1".into(), 346), ("number_0".into(), 378), ("number_5".into(), 398)]);
        // Zero minutes is one digit; 100 minutes or more grows to the left; 0:00 is still drawn.
        assert_eq!(sprites(&draws_at(0, true, 640, 480, &stop), "number_").len(), 3);
        assert_eq!(sprites(&draws_at(6000, true, 640, 480, &stop), "number_")[0].1, 326 - 20);
        // 320 x 240: stopwatch (160, 216); `2` at 176; colon x 192, y 220 and 228; seconds 196, 208.
        let small = |k: &str| match k {
            "stopwatch" => Some(UVec2::splat(16)),
            "number_0" => Some(UVec2::new(12, 16)),
            _ => None,
        };
        let d = draws_at(164, true, 320, 240, &small);
        let at: Vec<_> = d.iter().map(|d| match d {
            Draw::Sprite { pic, x, y, .. } => (pic.clone(), *x, *y),
            Draw::Add { rect, .. } => ("sq".into(), rect[0], rect[1]),
            _ => panic!(),
        }).collect();
        assert_eq!(at, vec![
            ("stopwatch".into(), 160, 216), ("number_2".into(), 176, 216), ("sq".into(), 192, 220), ("sq".into(), 192, 228),
            ("number_4".into(), 196, 216), ("number_4".into(), 208, 216),
        ]);
    }

    /// hud.md's test cases: the 320 layout's worked positions at 320 x 240.
    #[test]
    fn the_320_layout_sits_by_the_formulas() {
        let small = |k: &str| match k {
            "number_0" => Some(UVec2::new(12, 16)),
            "cross" | "suit_empty" | "suit_full" | "suithelmet_empty" | "suithelmet_full" | "stopwatch" => Some(UVec2::splat(16)),
            k if k.starts_with("bucket") => Some(UVec2::splat(12)),
            "selection" => Some(UVec2::new(80, 20)),
            k if k.starts_with("menu") => Some(UVec2::new(80, 20)),
            k if k.starts_with("ammo:") => Some(UVec2::splat(24)),
            _ => None,
        };
        let f = Frame { health: Some(100), armour: (50, false), weapon_id: "cs16:weapon_ak47".into(), ammo: Ammo::Magazine { clip: 30, reserve: 90 }, ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 320, 240, small);
        assert_eq!(sprites(&d, "cross"), vec![("cross".into(), 8, 216)]);
        let n = sprites(&d, "number_");
        assert_eq!(n[..3].iter().map(|s| (s.1, s.2)).collect::<Vec<_>>(), vec![(22, 219), (34, 219), (46, 219)]);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [64, 219, 1, 16], .. })), "the bar");
        // Armour icon at (64, 214), its number from x 80.
        assert_eq!(sprites(&d, "suit_empty"), vec![("suit_empty".into(), 64, 214)]);
        assert_eq!(n[3].1, 80 + 12);
        // Ammunition: the clip from 200, the bar at 242, the reserve from 250.
        assert_eq!(n[5].1, 200 + 12);
        assert!(d.iter().any(|d| matches!(d, Draw::Fill { rect: [242, 219, 1, 16], .. })));
        assert_eq!(n[7].1, 250 + 12);
    }

    /// hud.md's menu test cases: 640 x 480 and 320 x 240.
    #[test]
    fn the_menu_sits_right_of_the_radar() {
        let big = |k: &str| match k {
            "number_0" => Some(UVec2::new(20, 25)),
            k if k.starts_with("bucket") => Some(UVec2::splat(20)),
            "selection" => Some(UVec2::new(170, 45)),
            k if k.starts_with("menu") => Some(UVec2::new(170, 45)),
            _ => None,
        };
        let w = |id: &str, empty, reserve, most| MenuWeapon { id: id.into(), empty, reserve, most };
        // Slot 2 holds only the USP, open: the numbers at 138, 163, 338, 363, 388.
        let menu = Menu { slots: vec![vec![w("cs16:weapon_ak47", false, 0, 0)], vec![w("cs16:weapon_usp", false, 0, 0)]], chosen: Some((1, 0)) };
        let f = Frame { health: Some(100), menu: Some(menu), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, big);
        let xs: Vec<i32> = sprites(&d, "bucket").iter().map(|s| s.1).collect();
        assert_eq!(xs, vec![138, 163, 338, 363, 388]);
        assert!(sprites(&d, "bucket").iter().all(|s| s.2 == 10));
        // The USP's lit picture and the selection at (163, 30), no brighter than the rest (192).
        assert_eq!(sprites(&d, "menu_s:cs16:weapon_usp"), vec![("menu_s:cs16:weapon_usp".into(), 163, 30)]);
        assert_eq!(sprites(&d, "selection"), vec![("selection".into(), 163, 30)]);
        assert!(d.iter().any(|d| matches!(d, Draw::Sprite { pic, alpha: 192, .. } if pic == "menu_s:cs16:weapon_usp")));
        // The AK in closed slot 1: a 20 x 20 box at (138, 30) at alpha 128, additive.
        assert!(d.iter().any(|d| matches!(d, Draw::Add { rect: [138, 30, 20, 20], colour: HUD_COLOUR, alpha: 128 })));
        // Slot 1 open, the AK with 45 of 90: the bar at (148, 30), 10 greenish, 10 HUD colour at 128.
        let menu = Menu { slots: vec![vec![w("cs16:weapon_ak47", false, 45, 90)]], chosen: Some((0, 0)) };
        let f = Frame { health: Some(100), menu: Some(menu), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, big);
        assert!(d.contains(&Draw::Add { rect: [148, 30, 10, 4], colour: MENU_BAR_COLOUR, alpha: 255 }));
        assert!(d.contains(&Draw::Add { rect: [158, 30, 10, 4], colour: HUD_COLOUR, alpha: 128 }));
        // A weapon without ammunition: the reddish colour at 128 in the open slot; no reserve, no bar.
        let menu = Menu { slots: vec![vec![w("cs16:weapon_ak47", false, 0, 90), w("cs16:weapon_deagle", true, 0, 35)], vec![]], chosen: Some((0, 0)) };
        let f = Frame { health: Some(100), menu: Some(menu), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 640, 480, big);
        assert!(d.iter().any(|d| matches!(d, Draw::Sprite { pic, colour: EMPTY_COLOUR, alpha: 128, y: 80, .. } if pic == "menu:cs16:weapon_deagle")));
        assert!(!d.iter().any(|d| matches!(d, Draw::Add { colour: MENU_BAR_COLOUR, .. })));
        // 320 x 240: columns from 74, the pictures from y 22, 25 apart, the bar 10 x 2 at x + 5.
        let small = |k: &str| match k {
            "number_0" => Some(UVec2::new(12, 16)),
            k if k.starts_with("bucket") => Some(UVec2::splat(12)),
            "selection" => Some(UVec2::new(80, 20)),
            k if k.starts_with("menu") => Some(UVec2::new(80, 20)),
            _ => None,
        };
        let menu = Menu { slots: vec![vec![w("cs16:weapon_ak47", false, 45, 90), w("cs16:weapon_deagle", false, 0, 0)]], chosen: Some((0, 0)) };
        let f = Frame { health: Some(100), menu: Some(menu), ..default() };
        let d = draws(&f, &Fades::default(), &[0.0; 4], 320, 240, small);
        assert_eq!(sprites(&d, "bucket")[0], ("bucket1".into(), 74, 10));
        assert_eq!(sprites(&d, "menu:cs16:weapon_deagle"), vec![("menu:cs16:weapon_deagle".into(), 74, 47)]);
        assert!(d.contains(&Draw::Add { rect: [79, 22, 5, 2], colour: MENU_BAR_COLOUR, alpha: 255 }));
        assert_eq!(sprites(&d, "bucket")[1].1, 74 + 80 + 5);
    }

    #[test]
    fn digits_leave_leading_places_empty() {
        assert_eq!(digits(0, 0), vec![(0, 40)]);
        assert_eq!(digits(42, 0), vec![(4, 20), (2, 40)]);
        assert_eq!(digits(1234, 0).len(), 4);
    }

    /// hud.md's test cases 13 and 14, and the arcs' places at 640 x 480.
    #[test]
    fn pain_arcs_light_by_direction() {
        // Looking down -Z (yaw 0); the source straight ahead.
        let s = hit_arcs([0.0; 4], Vec3::new(0.0, 0.0, -1.0), 0.0, false);
        assert_eq!(s, [1.0, 0.0, 0.0, 0.0]);
        // From the right (+X).
        assert_eq!(hit_arcs([0.0; 4], Vec3::X, 0.0, false), [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(hit_arcs([0.0; 4], Vec3::X, 0.0, true), [1.0; 4]);
        // Drawn while above 0.4: 0.3 s at 2 a second.
        assert!(1.0 - PAIN_DRAIN * 0.29 > PAIN_SHOWN && 1.0 - PAIN_DRAIN * 0.31 < PAIN_SHOWN);
        let d = draws(&Frame::default(), &Fades::default(), &[1.0; 4], 640, 480, sizes);
        let places: Vec<_> = sprites(&d, "pain_").into_iter().map(|s| (s.1, s.2)).collect();
        assert_eq!(places, vec![(256, 96), (416, 176), (256, 336), (176, 176)]);
    }

    #[test]
    fn a_kill_icon_is_tinted_by_its_brightness() {
        let p = UiImage { width: 2, height: 1, rgba8: vec![0, 0, 0, 255, 200, 100, 50, 255] };
        assert_eq!(tinted(&p, KILL_ICON_COLOUR).rgba8, vec![255, 80, 0, 0, 255, 80, 0, 200]);
    }
}
