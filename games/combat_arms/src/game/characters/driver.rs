//! The third-person animation driver (specs/combat_arms/characters.md
//! sections 4 to 7 and 9): each update the four trackers advance, the
//! posture (upper body) and movement (legs) are decided from the weapon
//! status, the fire buttons and the movement flags, the animation names
//! are built from the weapon class, stance, action and direction, and
//! the trackers are started, restarted and kept in step by section 7's
//! rules, on LithTech's own millisecond trackers (`lithtech_formats::
//! anim::Tracker`) over the body's combined animation list.
//!
//! The infected hands' classes (39 `VUM...`, 47 `VGUM...`: the knife
//! names with code `M`, prefixed) and the powered suits' robot classes
//! (58-67, characters.md 13.7: their names per posture and direction from
//! data/npc_bodies.toml, `families::RobotClass`) build their own names; a
//! body's family may give its own hit names, and a boss's runs a minimal
//! mapping instead (`update_boss`, 13.8).
//!
//! A name the library lacks starts nothing (Quirk 1). Without a body
//! model the library is `spec_library`: every name section 6 can build,
//! with the spec's measured lengths, minus the names it says the US
//! libraries lack.

use std::sync::{Arc, OnceLock};

use lithtech_formats::{
    anim::{AnimList, Entry, Tracker},
    ltb::{Animation, Key},
};
use serde::{Deserialize, Serialize};

/// The upper body's action (5, "Posture").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Posture {
    #[default]
    Aim,
    PreFire,
    Fire,
    PostFire,
    Select,
    Reload,
    Hit,
    Hit2,
    AltFire,
}

impl Posture {
    /// `HHAniFrame_#`'s posture index (7).
    pub fn frame_index(self) -> usize {
        match self {
            Posture::Aim => 0,
            Posture::PreFire => 1,
            Posture::Fire => 2,
            Posture::PostFire => 3,
            Posture::Select => 4,
            Posture::Reload => 5,
            Posture::Hit => 6,
            Posture::Hit2 => 7,
            Posture::AltFire => 9,
        }
    }

    fn looping(self) -> bool {
        self == Posture::Aim
    }
}

/// The legs' movement (5, "Movement").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Movement {
    #[default]
    Walk,
    Run,
    Sprint,
    Jump,
}

/// The direction from the movement keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    None,
    F,
    B,
    L,
    R,
    FL,
    FR,
    BL,
    BR,
}

impl Direction {
    /// From the keys: right strafe with forward or backward gives the
    /// diagonals; both strafes held, the left one wins.
    pub fn of(forward: bool, back: bool, left: bool, right: bool) -> Self {
        let side = if left {
            Some(true)
        } else if right {
            Some(false)
        } else {
            None
        };
        // The keys apply forward, then backward (which overrides it), then
        // the strafes; within a strafe the forward key is tested first.
        match (side, forward, back) {
            (None, _, true) => Direction::B,
            (None, true, false) => Direction::F,
            (None, false, false) => Direction::None,
            (Some(l), true, _) => {
                if l {
                    Direction::FL
                } else {
                    Direction::FR
                }
            }
            (Some(l), false, true) => {
                if l {
                    Direction::BL
                } else {
                    Direction::BR
                }
            }
            (Some(l), false, false) => {
                if l {
                    Direction::L
                } else {
                    Direction::R
                }
            }
        }
    }

    /// The upper body's direction suffix: diagonals take forward or back.
    fn upper(self) -> &'static str {
        match self {
            Direction::None => "S",
            Direction::F | Direction::FL | Direction::FR => "F",
            Direction::B | Direction::BL | Direction::BR => "B",
            Direction::L => "L",
            Direction::R => "R",
        }
    }

    fn legs(self) -> &'static str {
        match self {
            Direction::None => "",
            Direction::F => "F",
            Direction::B => "B",
            Direction::L => "L",
            Direction::R => "R",
            Direction::FL => "FL",
            Direction::FR => "FR",
            Direction::BL => "BL",
            Direction::BR => "BR",
        }
    }
}

/// The weapon status the client reports (5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    None,
    Select,
    Deselect,
    Idle,
    Hit,
    /// The weapon reloads: no firing choice until the reload ends.
    Reloading,
}

/// The jump status (5, "Movement").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Jump {
    #[default]
    None,
    Jumped,
    Falling,
}

/// What the driver is given each update.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// The current weapon's `AniType` (14 without a weapon) and its leg
    /// set (`LowerAniType`).
    pub class: i32,
    pub legs: i32,
    /// A status reported since the last update.
    pub status: Option<Status>,
    /// The weapon is reloading, and whether that reload may finish (it
    /// left the reload state).
    pub reloading: bool,
    pub reload_may_finish: bool,
    pub fire: bool,
    pub alt_fire: bool,
    /// The current ammo count is 0.
    pub empty: bool,
    pub run: bool,
    pub sprint: bool,
    pub zoomed: bool,
    /// A jump started, or the player landed, since the last update.
    pub jumped: bool,
    pub landed: bool,
    /// The footing test failed this movement tick (characters.md
    /// Corrections 3): the client did not jump, and no floor lies within
    /// 150 u below the hull centre; it marks the character falling.
    pub no_footing: bool,
    /// On the ground now (a falling character landed).
    pub grounded: bool,
    /// The server accepted a shot (or a throw) since the last update
    /// (Corrections 4: the sniper's and land mine's fire pose).
    pub shot: bool,
    pub crouch_key: bool,
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    /// The crouch fraction r.
    pub crouch: f32,
    /// The last damage was above 0, and spawn protection.
    pub damaged: bool,
    pub protected: bool,
    /// The hit reaction's coin: `D_HIT` when set, else `D_HITT`.
    pub coin: bool,
    /// `HHAniFrame_0..9` (ms, 0: none) and the weapon rates per posture
    /// (reload, select, fire) for the upper rate (7).
    pub frames: Vec<u32>,
    pub rates: Rates,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            class: 0,
            legs: 0,
            status: None,
            reloading: false,
            reload_may_finish: false,
            fire: false,
            alt_fire: false,
            empty: false,
            run: true,
            sprint: false,
            zoomed: false,
            jumped: false,
            landed: false,
            no_footing: false,
            grounded: false,
            shot: false,
            crouch_key: false,
            forward: false,
            back: false,
            left: false,
            right: false,
            crouch: 0.0,
            damaged: false,
            protected: false,
            coin: true,
            frames: Vec::new(),
            rates: Rates::default(),
        }
    }
}

/// The weapon rate r_w of each posture kind (7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub reload: f32,
    pub select: f32,
    pub fire: f32,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            reload: 1.0,
            select: 1.0,
            fire: 1.0,
        }
    }
}

/// The server's reload timer (7, The server's reload; cases 48-50), ms:
/// none when the weapon's `HHAniFrame_5` is set (`frame` > 0: the
/// third-person reload animation ends it); else the reload animation's
/// length `length` over the server's reload rate 1.2 min(r, 2), truncated
/// to whole ms; 500 ms when the class has no reload animation.
pub fn server_reload_timer(length: Option<u32>, rate: f32, frame: u32) -> Option<u32> {
    if frame > 0 {
        return None;
    }
    Some(match length {
        Some(l) => (l as f64 / (1.2 * rate.min(2.0) as f64)).floor() as u32,
        None => SERVER_RELOAD_FALLBACK,
    })
}

/// The server's reload timer without a reload animation, ms.
pub const SERVER_RELOAD_FALLBACK: u32 = 500;

/// "Fire held" for the driver (5, Open question 17): the server's is the
/// frame's raw fire flag (cleared while use is held); a client's own
/// body's is masked by its weapon being in a firing state (not reloading,
/// not empty).
pub fn fire_held(fire: bool, use_key: bool, server: bool, magazine: bool, reloading: bool, empty: bool) -> bool {
    fire && !use_key && (server || !(magazine && (reloading || empty)))
}

/// The driver's "ammo count 0" (Corrections 6): the weapon's total
/// ammunition (the clip and the reserve together), not the clip.
pub fn ammo_exhausted(clip: u32, reserve: u32) -> bool {
    clip + reserve == 0
}

/// The upper rate (7): L r_w / F for a weapon whose `HHAniFrame` for the
/// posture is F > 0, else 1.
pub fn upper_rate(length: u32, rate: f32, frame: u32) -> f32 {
    if frame > 0 { length as f32 * rate / frame as f32 } else { 1.0 }
}

/// Which animations a state names (6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Names {
    pub upper: String,
    /// The walk and sprint variants, used when they exist.
    pub upper_walk: Option<String>,
    pub upper_sprint: Option<String>,
    pub lower: Option<String>,
    pub crouch: String,
}

/// The class code and its reload/fire digit (6, "Class code").
pub fn class_code(class: i32) -> (&'static str, &'static str) {
    match class {
        0 | 1 => ("RF", ""),
        2 => ("RH", ""),
        3 => ("SM", ""),
        4 => ("SH", ""),
        5 => ("SN", ""),
        6 => ("SG", ""),
        7 => ("G", ""),
        8 => ("M", ""),
        9 => ("V", ""),
        10 => ("P", ""),
        11 => ("T", ""),
        12 => ("RL", ""),
        13 => ("TB", ""),
        14 => ("K", ""),
        15 => ("F", ""),
        16 => ("BK", ""),
        17 => ("FT", ""),
        18 => ("HS", ""),
        19 => ("RW", ""),
        20 => ("LM", ""),
        21 => ("CM", ""),
        22 => ("CT", ""),
        23 => ("RA", ""),
        24 => ("P", "1"),
        25 => ("P9", ""),
        26 => ("SM", "2"),
        27 => ("SG", "1"),
        28 => ("GG", ""),
        29 => ("SG", "2"),
        _ => ("RF", ""),
    }
}

fn knife(class: i32) -> bool {
    matches!(class, 14 | 16 | 39 | 47)
}

/// Whether class `class` has an alt-fire of its own (the knives, the
/// infected hands, the melee suits' second swing).
pub fn has_alt_fire(class: i32) -> bool {
    knife(class) || matches!(class, 61 | 67)
}

/// The infected hands' classes (characters.md 12.6, 13.5): the knife
/// names with code `M`, prefixed `V` (39, `VUM...`) or `VG` (47, the club
/// zombie's `VGUM...`).
fn infected_prefix(class: i32) -> Option<&'static str> {
    match class {
        39 => Some("V"),
        47 => Some("VG"),
        _ => None,
    }
}

/// A robot class's names (characters.md 13.7, data/npc_bodies.toml): the
/// posture's row by direction (diagonals as forward or back), its run
/// and walk forms, and the posture's sprint name.
fn robot_names(t: &super::families::RobotClass, posture: Posture, dir: Direction) -> (String, Option<String>, Option<String>) {
    let i = match dir {
        Direction::None => 0,
        Direction::F | Direction::FL | Direction::FR => 1,
        Direction::B | Direction::BL | Direction::BR => 2,
        Direction::L => 3,
        Direction::R => 4,
    };
    let (row, sprint) = match posture {
        Posture::Hit => return ("D_HIT".into(), None, None),
        Posture::Hit2 => return ("D_HITT".into(), None, None),
        Posture::Aim => (&t.aim, &t.sprint.aim),
        Posture::Fire | Posture::PreFire | Posture::PostFire => (&t.fire, &t.sprint.fire),
        Posture::Select => (&t.select, &t.sprint.select),
        Posture::Reload => (&t.reload, &t.sprint.reload),
        Posture::AltFire => (t.alt_fire.as_ref().unwrap_or(&t.aim), &t.sprint.aim),
    };
    let (run, walk) = super::families::forms(&row[i]);
    (run, Some(walk), Some(sprint.clone()))
}

fn thrown(class: i32) -> bool {
    matches!(class, 11 | 21)
}

/// The upper-body names of a state (6): the run name and its walk and
/// sprint variants. The digit classes (Open question 18): the revolver
/// (24) and the FAMAS (26) carry their digit on the reload only, the
/// double barrel (27) and the snowball gun (29) on fire, alt-fire, select
/// and reload (their alt-fire is their fire).
pub fn upper_names(class: i32, posture: Posture, dir: Direction) -> (String, Option<String>, Option<String>) {
    if let Some(t) = super::families::families().robot(class) {
        return robot_names(t, posture, dir);
    }
    if let Some(prefix) = infected_prefix(class) {
        if matches!(posture, Posture::Hit | Posture::Hit2) {
            return coded_names(class, "M", "", posture, dir);
        }
        let (run, walk, sprint) = coded_names(class, "M", "", posture, dir);
        let p = |n: String| format!("{prefix}{n}");
        return (p(run), walk.map(p), sprint.map(p));
    }
    let (code, digit) = class_code(class);
    coded_names(class, code, digit, posture, dir)
}

/// `upper_names` for a class of code `code` and digit `digit` (6).
fn coded_names(class: i32, code: &str, digit: &str, posture: Posture, dir: Direction) -> (String, Option<String>, Option<String>) {
    let all_digit = matches!(class, 27 | 29);
    let fire_digit = if all_digit { digit } else { "" };
    let d = dir.upper();
    let moving = dir != Direction::None;
    // Sniper select and reload reuse the rifle's.
    let sr = if class == 5 { "RF" } else { code };
    let name = |stance: &str, action: &str, suffix: &str| format!("U{code}{stance}{action}{suffix}");
    let posture = if all_digit && posture == Posture::AltFire { Posture::Fire } else { posture };
    match posture {
        Posture::Hit => ("D_HIT".into(), None, None),
        Posture::Hit2 => ("D_HITT".into(), None, None),
        Posture::Aim => (name("R", "Am", d), Some(name("W", "Am", d)), Some(name("RR", "Am", "F"))),
        Posture::Fire | Posture::PreFire | Posture::PostFire => {
            let part = match posture {
                Posture::PreFire => "_A",
                Posture::Fire if thrown(class) => "_B",
                Posture::PostFire => "_C",
                _ => "",
            };
            if knife(class) {
                (name("R", "Fi", "S"), None, None)
            } else {
                let suffix = format!("{d}{fire_digit}{part}");
                (name("R", "Fi", &suffix), Some(name("W", "Fi", &suffix)), Some(name("RR", "Am", "F")))
            }
        }
        Posture::AltFire => {
            if knife(class) {
                (name("R", "Fi", "S2"), None, None)
            } else {
                (name("R", "Am", "S"), None, None)
            }
        }
        Posture::Select | Posture::Reload => {
            let action = if posture == Posture::Select { "Se" } else { "Re" };
            let digit = if posture == Posture::Reload { digit } else { fire_digit };
            if !moving {
                (format!("U{sr}RS{action}S{digit}"), None, Some(format!("U{sr}RR{action}S{digit}")))
            } else if class == 1 && posture == Posture::Reload {
                ("URBRReS".into(), None, Some(format!("U{sr}RR{action}S{digit}")))
            } else {
                (format!("U{sr}R{action}S{digit}"), None, Some(format!("U{sr}RR{action}S{digit}")))
            }
        }
    }
}

/// The legs' names (6): the lower tracker's (None: sprint in a direction
/// without one) and the crouch tracker's, with the leg set's prefix.
pub fn leg_names(legs: i32, movement: Movement, dir: Direction) -> (Option<String>, String) {
    let prefix = match legs {
        1 => "V",
        2 => "VG",
        _ => "",
    };
    let d = dir.legs();
    let lower = match (movement, dir) {
        (Movement::Walk | Movement::Run, Direction::None) => Some("LST".to_string()),
        (Movement::Walk, _) => Some(format!("LW{d}")),
        (Movement::Run, _) => Some(format!("LR{d}")),
        (Movement::Jump, Direction::None) => Some("LSJ".into()),
        (Movement::Jump, Direction::F) => Some("LJJ".into()),
        (Movement::Jump, Direction::FL) => Some("LJJL".into()),
        (Movement::Jump, Direction::FR) => Some("LJJR".into()),
        (Movement::Jump, _) => Some(format!("LJ{d}")),
        (Movement::Sprint, Direction::F) => Some("LRRF".into()),
        (Movement::Sprint, Direction::FL) => Some("LRRL".into()),
        (Movement::Sprint, Direction::FR) => Some("LRRR".into()),
        (Movement::Sprint, _) => None,
    };
    let crouch = format!("LC{d}");
    (lower.map(|l| format!("{prefix}{l}")), format!("{prefix}{crouch}"))
}

/// The four values the driver last acted on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decided {
    pub class: i32,
    pub posture: Posture,
    pub movement: Movement,
    pub direction: Direction,
}

/// One character's driver.
#[derive(Clone, Debug)]
pub struct Driver {
    pub list: Arc<AnimList>,
    pub main: Tracker,
    pub upper: Tracker,
    pub lower: Tracker,
    pub crouch: Tracker,
    /// The values of the last update (5: remembered as "last").
    pub last: Decided,
    /// The posture whose animation the upper tracker plays (None: none
    /// started).
    pub playing: Option<Posture>,
    pub status: Status,
    pub jump: Jump,
    /// The reload in progress was finished by its animation.
    reload_finished: bool,
    /// The body (file) is new: the next update starts every tracker.
    fresh: bool,
    /// The emote playing on the upper tracker (9), its entry. The lower
    /// tracker's copy is replaced by the legs on the next update.
    pub emote: Option<usize>,
    /// The entry of the last upper animation the driver started (the
    /// upper tracker is started again only when the class or posture
    /// changed, or this name differs while aiming or firing).
    started: Option<usize>,
    /// The body's family (characters.md 13.4; None: a player body's
    /// rules): its hit names, a boss's mapping.
    pub family: Option<&'static super::families::Family>,
    /// A boss body's action (`update_boss`).
    pub boss: BossAction,
}

/// What a boss body plays (13.8's minimal mapping).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BossAction {
    #[default]
    Idle,
    Walk,
    FireStart,
    FireLoop,
    FireEnd,
    Melee,
    Hit,
}

/// The emote menu's choices 1-4 (9: Hunted only; `DT` is disabled).
pub const EMOTES: [&str; 4] = ["Insult", "Surrender", "Insult_Dance", "Salute"];

/// The hit kick on a remote character (9, case 52): the displayed pitch
/// offset (radians, upward) of the k-th damage notice since the counter
/// last restarted, and the counter after it: 0.1 k for k = 1..5, 0.1 (10 -
/// k) for 6..10; the 11th adds nothing and restarts the counter.
pub fn hit_kick(count: u32) -> (f32, u32) {
    let k = count + 1;
    match k {
        1..=5 => (0.1 * k as f32, k),
        6..=10 => (0.1 * (10 - k) as f32, k),
        _ => (0.0, 0),
    }
}

impl Driver {
    /// A driver on `list`: the main tracker on entry 0, looping.
    pub fn new(list: Arc<AnimList>) -> Self {
        Self {
            list,
            main: Tracker::on(0, true),
            upper: Tracker::on(0, true),
            lower: Tracker::on(0, true),
            crouch: Tracker::on(0, true),
            last: Decided {
                class: 0,
                ..Default::default()
            },
            playing: None,
            status: Status::None,
            jump: Jump::None,
            reload_finished: false,
            fresh: true,
            emote: None,
            started: None,
            family: None,
            boss: BossAction::Idle,
        }
    }

    /// Start emote `name` (9): only while aiming with no direction,
    /// walking or running, and with nothing else emoting; both the upper
    /// and the lower tracker play it once. False when it can't (or the
    /// library lacks it: the US male's `Insult` and `Salute`).
    pub fn start_emote(&mut self, name: &str) -> bool {
        let quiet = self.last.posture == Posture::Aim
            && self.last.direction == Direction::None
            && matches!(self.last.movement, Movement::Walk | Movement::Run);
        let Some(e) = self.find(name) else { return false };
        if !quiet || self.emote.is_some() {
            return false;
        }
        for t in [&mut self.upper, &mut self.lower] {
            t.start(e);
            t.looping = false;
        }
        self.emote = Some(e);
        // Same-name aim after it must not restart the upper tracker.
        true
    }

    /// The entry named `name`, if the library has it.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.list.find(name)
    }

    /// The name of the entry a tracker plays.
    pub fn name_of(&self, t: &Tracker) -> &str {
        self.list.entries.get(t.current).map_or("", |e| e.name())
    }

    fn done(&self, t: &Tracker) -> bool {
        t.done(&self.list)
    }

    /// The upper animation of posture `p` plays and is done / still plays.
    fn finished(&self, p: Posture) -> bool {
        self.playing == Some(p) && self.done(&self.upper)
    }

    fn running(&self, p: Posture) -> bool {
        self.playing == Some(p) && !self.done(&self.upper)
    }

    /// Advance every tracker by `ms` (the engine's per-frame advance).
    pub fn advance(&mut self, ms: u32) {
        let list = self.list.clone();
        for t in [&mut self.main, &mut self.upper, &mut self.lower, &mut self.crouch] {
            t.update(&list, ms, &mut |_| {});
        }
    }

    /// The posture (5).
    fn posture(&mut self, i: &Inputs) -> Posture {
        let last = self.last.posture;
        let mut posture = Posture::Aim;
        if self.status == Status::Idle {
            self.status = Status::None;
            return Posture::Aim;
        }
        if !i.reloading {
            self.reload_finished = false;
            if self.status == Status::Reloading {
                self.status = Status::None;
            }
        }
        // 2. Reloading.
        if i.reloading && !self.reload_finished {
            if self.status == Status::None {
                self.status = Status::Reloading;
            }
            if i.reload_may_finish || self.finished(Posture::Reload) {
                self.reload_finished = true;
                if self.status == Status::Reloading {
                    self.status = Status::None;
                }
            } else if last == Posture::PostFire && self.running(Posture::PostFire) {
                posture = last;
            } else {
                posture = Posture::Reload;
            }
        }
        // 3. Select, deselect, hits.
        if self.status == Status::Select || self.finished(Posture::PostFire) || self.running(Posture::Select) {
            posture = Posture::Select;
            // Done, or (a library without the name) never started.
            let none = last == Posture::Select && self.playing != Some(Posture::Select);
            if self.finished(Posture::Select) || none {
                self.status = Status::None;
            }
        } else if self.status == Status::Deselect {
            self.status = Status::None;
        } else if self.status == Status::Hit || self.running(Posture::Hit) || self.running(Posture::Hit2) {
            if self.running(Posture::Hit) || self.running(Posture::Hit2) {
                posture = last;
            } else if matches!(last, Posture::Hit | Posture::Hit2) && self.done(&self.upper) {
                self.status = Status::None;
            } else if i.damaged && !i.protected {
                posture = if i.coin { Posture::Hit } else { Posture::Hit2 };
            }
        }
        // 4. The firing choice.
        if self.status == Status::None {
            posture = self.firing(i, posture);
            // No fire pose while the legs play the sprint (Corrections 5).
            if matches!(posture, Posture::Fire | Posture::AltFire)
                && self.last.movement == Movement::Sprint
                && !self.done(&self.lower)
            {
                posture = Posture::Aim;
            }
            if i.empty && self.done(&self.upper) {
                posture = Posture::Aim;
            }
        }
        posture
    }

    fn firing(&self, i: &Inputs, current: Posture) -> Posture {
        let last = self.last.posture;
        let wait = |p: Posture| matches!(last, Posture::Reload | Posture::Select | Posture::AltFire) && self.running(last) && p != last;
        match i.class {
            // The fire pose follows each accepted shot, not the button
            // (Corrections 4).
            5 | 20 => {
                if i.shot || self.running(Posture::Fire) {
                    Posture::Fire
                } else {
                    Posture::Aim
                }
            }
            11 | 21 => match last {
                Posture::Aim if i.fire && current != Posture::Select => Posture::PreFire,
                Posture::PreFire if !self.done(&self.upper) => Posture::PreFire,
                Posture::PreFire => Posture::Fire,
                Posture::Fire if i.fire => Posture::Fire,
                Posture::Fire => Posture::PostFire,
                Posture::PostFire if !self.done(&self.upper) => Posture::PostFire,
                _ => Posture::Aim,
            },
            _ => {
                if i.fire || self.running(Posture::Fire) {
                    if wait(Posture::Fire) { last } else { Posture::Fire }
                } else if i.alt_fire || self.running(Posture::AltFire) {
                    if wait(Posture::AltFire) { last } else { Posture::AltFire }
                } else {
                    Posture::Aim
                }
            }
        }
    }

    /// The movement and direction (5), or None to leave the jump's
    /// direction as it was.
    fn movement(&mut self, i: &Inputs) -> (Movement, Direction) {
        if i.landed {
            self.jump = Jump::None;
        }
        if i.grounded && self.jump == Jump::Falling && !i.jumped {
            self.jump = Jump::None;
        }
        if i.jumped {
            self.jump = Jump::Jumped;
        } else if i.no_footing && self.jump != Jump::Falling {
            // Walking off a real drop (Corrections 3).
            self.jump = Jump::Falling;
        }
        // Crouching in the air clears the status; only a new jump sets it
        // again (Corrections 7).
        if self.jump != Jump::None && i.crouch_key {
            self.jump = Jump::None;
        }
        match self.jump {
            Jump::Jumped => {
                if self.last.movement == Movement::Jump && self.done(&self.lower) {
                    self.jump = Jump::Falling;
                    return (Movement::Walk, Direction::None);
                }
                // A sprinting jump plays the run jump (Quirk 3).
                return (Movement::Jump, self.last.direction);
            }
            Jump::Falling => return (Movement::Walk, Direction::None),
            _ => {}
        }
        let movement = if i.sprint {
            Movement::Sprint
        } else if i.run && !i.zoomed {
            Movement::Run
        } else {
            Movement::Walk
        };
        (movement, Direction::of(i.forward, i.back, i.left, i.right))
    }

    /// Start `entry` on a tracker: switch when another plays; restart a
    /// done one when `restart`; else nothing.
    fn start(list: &AnimList, t: &mut Tracker, entry: usize, restart: bool) -> bool {
        if t.current != entry {
            t.start(entry);
            true
        } else if restart && t.done(list) {
            t.reset();
            true
        } else {
            false
        }
    }

    /// One update: advance by `ms`, decide, start (sections 5 to 7).
    pub fn update(&mut self, ms: u32, i: &Inputs) {
        if let Some(b) = self.family.and_then(|f| f.boss.as_ref()) {
            self.update_boss(ms, i, b);
            return;
        }
        self.advance(ms);
        if let Some(s) = i.status {
            self.status = s;
        }
        let posture = self.posture(i);
        let (movement, direction) = self.movement(i);
        let class = i.class;
        let (lower, crouch) = leg_names(i.legs, movement, direction);
        // Sprint in a direction without legs: nothing started or kept.
        let Some(lower) = lower else { return };
        // An emote stays on the upper tracker alone: the legs below replace
        // the lower tracker's copy at once, and after it ends the upper
        // tracker keeps its last pose until the driver next starts an
        // upper animation (9, Corrections 9).
        if let Some(e) = self.emote {
            let quiet = posture == Posture::Aim
                && direction == Direction::None
                && matches!(movement, Movement::Walk | Movement::Run);
            if !quiet || self.upper.current != e || self.done(&self.upper) {
                self.emote = None;
            }
        }
        let (mut run, mut walk, mut sprint) = upper_names(class, posture, direction);
        // A family's own hit reactions (13.8: the infected's `VD_HIT`).
        if let (Some([hit, hit2]), Posture::Hit | Posture::Hit2) = (self.family.and_then(|f| f.hits.as_ref()), posture) {
            run = if posture == Posture::Hit { hit.clone() } else { hit2.clone() };
            (walk, sprint) = (None, None);
        }
        let exists = |n: &Option<String>| n.as_deref().and_then(|n| self.find(n));
        let upper = match movement {
            Movement::Sprint => exists(&sprint).or_else(|| self.find(&run)),
            Movement::Walk => exists(&walk).or_else(|| self.find(&run)),
            _ => self.find(&run),
        };
        let list = self.list.clone();
        let fresh = std::mem::take(&mut self.fresh);
        let lower_entry = self.find(&lower);
        // A leg set without the crouch name crouches on the legs' own
        // animation (Corrections 13).
        if let Some(c) = self.find(&crouch).or(lower_entry) {
            self.crouch.looping = true;
            Self::start(&list, &mut self.crouch, c, true);
        }
        if let Some(l) = lower_entry {
            self.lower.looping = movement != Movement::Jump;
            Self::start(&list, &mut self.lower, l, true);
        }
        let changed = fresh || class != self.last.class || posture != self.last.posture;
        if let Some(u) = upper {
            let free = matches!(posture, Posture::Aim | Posture::Fire | Posture::AltFire);
            // The grenade class never restarts a finished animation.
            let restart = !thrown(class);
            // A held fire button plays its animation once (Corrections 1):
            // the tracker starts again only on a class or posture change,
            // a new accepted shot, or a different name while aiming or
            // firing.
            let shot = i.shot && matches!(class, 5 | 20) && posture == Posture::Fire;
            let go = changed || shot || (free && self.started != Some(u));
            if go {
                self.upper.looping = posture.looping();
            }
            let started = if changed || shot {
                Self::start(&list, &mut self.upper, u, restart || shot)
            } else if go {
                Self::start(&list, &mut self.upper, u, false)
            } else {
                false
            };
            if go {
                self.started = Some(u);
            }
            if started || (changed && self.upper.current == u) {
                self.playing = Some(posture);
            }
            if (changed || shot) && started {
                let length = list.entries[u].length();
                let frame = i.frames.get(posture.frame_index()).copied().unwrap_or(0);
                let rate = match posture {
                    Posture::Reload => i.rates.reload,
                    Posture::Select => i.rates.select,
                    Posture::Fire | Posture::PreFire | Posture::PostFire => i.rates.fire,
                    _ => 1.0,
                };
                self.upper.set_rate(upper_rate(length, rate, frame));
            }
        }
        // Keep the torso in step with the legs.
        if !fresh && (movement != self.last.movement || direction != self.last.direction) {
            let last = self.last.posture;
            let synced = match last {
                Posture::Fire | Posture::AltFire => false,
                Posture::Aim => true,
                _ => self.done(&self.upper),
            };
            if synced {
                let time = self.lower.time;
                self.upper.set_time(&list, time);
            }
        }
        self.crouch.multiplier = i.crouch.clamp(0.0, 1.0);
        self.last = Decided {
            class,
            posture,
            movement,
            direction,
        };
    }

    /// Change the library (a new body file): every tracker starts again
    /// on the next update.
    pub fn set_list(&mut self, list: Arc<AnimList>) {
        let family = self.family;
        *self = Self::new(list);
        self.family = family;
    }

    /// One update of a boss body (13.8's minimal mapping, `BossNames`):
    /// fire held plays the fire (its start, then its loop while held,
    /// then its end; one name loops while held), alt-fire the melee once,
    /// a hit (damage, not protected) the hit once, else walking (any
    /// direction) the walk, standing the idle; the same name on the upper
    /// and lower trackers (the body's lower set is empty, its upper set
    /// the whole body). A name the body lacks starts nothing.
    pub fn update_boss(&mut self, ms: u32, i: &Inputs, b: &super::families::BossNames) {
        self.advance(ms);
        if let Some(s) = i.status {
            self.status = s;
        }
        let done = self.done(&self.upper);
        let three = b.fire.len() == 3;
        let hit = self.status == Status::Hit && i.damaged && !i.protected && b.hit.is_some();
        self.status = Status::None;
        use BossAction as A;
        let fire = i.fire && !b.fire.is_empty();
        let next = match self.boss {
            A::FireStart if done => A::FireLoop,
            A::FireStart => A::FireStart,
            A::FireLoop if !fire && three => A::FireEnd,
            A::FireLoop if fire => A::FireLoop,
            A::FireEnd if !done => A::FireEnd,
            A::Melee | A::Hit if !done => self.boss,
            _ if fire => {
                if three {
                    A::FireStart
                } else {
                    A::FireLoop
                }
            }
            _ if i.alt_fire && b.melee.is_some() => A::Melee,
            _ if hit => A::Hit,
            _ if (i.forward || i.back || i.left || i.right) && b.walk.is_some() => A::Walk,
            _ => A::Idle,
        };
        let name = match next {
            A::Idle => Some(&b.idle),
            A::Walk => b.walk.as_ref(),
            A::FireStart => b.fire.first(),
            A::FireLoop => b.fire.get(usize::from(three)),
            A::FireEnd => b.fire.get(2),
            A::Melee => b.melee.as_ref(),
            A::Hit => b.hit.as_ref(),
        };
        let changed = next != self.boss || std::mem::take(&mut self.fresh);
        self.boss = next;
        let Some(entry) = name.and_then(|n| self.find(n)) else { return };
        let looping = matches!(next, A::Idle | A::Walk | A::FireLoop);
        let list = self.list.clone();
        for t in [&mut self.upper, &mut self.lower] {
            t.looping = looping;
            if changed && t.current == entry && !looping {
                t.reset();
            } else {
                Self::start(&list, t, entry, looping);
            }
        }
        let posture = match next {
            A::FireStart | A::FireLoop | A::FireEnd => Posture::Fire,
            A::Melee => Posture::AltFire,
            A::Hit => Posture::Hit,
            _ => Posture::Aim,
        };
        self.playing = Some(posture);
        self.last = Decided {
            class: i.class,
            posture,
            movement: if next == A::Walk { Movement::Run } else { Movement::Walk },
            direction: Direction::of(i.forward, i.back, i.left, i.right),
        };
    }
}

/// Lengths (ms) the spec measured on the US male library (7, 10), for
/// the library used without a body model.
const LENGTHS: &[(&str, u32)] = &[
    ("LST", 4000),
    ("LC", 4000),
    ("LWF", 1000),
    ("LRF", 600),
    ("LRB", 600),
    ("LRL", 600),
    ("LRR", 600),
    ("LCF", 1400),
    ("LSJ", 666),
    ("LJJ", 666),
    ("LJB", 666),
    ("LRRF", 500),
    ("URFRAmS", 4000),
    ("URFRFiS", 100),
    ("URFRSSeS", 733),
    ("URFRSReS", 1866),
    ("UKRFiS", 500),
    ("UKRFiS2", 800),
    ("UTRFiS_A", 267),
    ("UTRFiS_B", 1),
    ("UTRFiS_C", 900),
    ("D_HIT", 467),
    ("D_HITT", 400),
    ("DH", 1433),
    ("DH_1", 1700),
    ("DT", 3033),
    ("DT_1", 1333),
    ("DHB", 1566),
    ("DTB", 1766),
    ("DLSH", 1766),
    ("DLT", 1333),
    ("DRSH", 1233),
    ("DRT", 1433),
    ("DC", 1900),
    ("D_S", 3033),
    ("D_P", 5633),
    ("D_W", 7800),
    ("D_BO_F", 1500),
    ("NS1", 5566),
    ("NS2", 3166),
];

/// Names the spec says the US libraries lack (6, Quirk 1), and whole
/// class families (`SH`, `G`, `M`).
fn missing(name: &str) -> bool {
    const ABSENT: &[&str] = &[
        "URFWAmS", "URFRFiF", "URFRFiB", "URFRFiL", "URFRFiR", "URFWFiF", "URFWFiB", "URFWFiL", "URFWFiR",
    ];
    ABSENT.iter().any(|a| a.eq_ignore_ascii_case(name))
        || ["USH", "UG", "UM"].iter().any(|f| {
            name.strip_prefix(f)
                .is_some_and(|rest| rest.starts_with(['R', 'W', 'C']))
        })
}

/// The length the stand-in library gives a name: the spec's measured one,
/// else the same kind's (legs 600, crouch 1400, jumps 666, aims 4000,
/// fires 100, selects 733, reloads 1866, deaths 1900).
fn length_of(name: &str) -> u32 {
    if let Some((_, l)) = LENGTHS.iter().find(|(n, _)| *n == name) {
        return *l;
    }
    if name.starts_with("LC") {
        1400
    } else if name.starts_with("LJ") || name.starts_with("LSJ") {
        666
    } else if name.starts_with("LRR") {
        500
    } else if name.starts_with('L') {
        600
    } else if name.contains("Am") {
        4000
    } else if name.contains("Fi") {
        100
    } else if name.contains("Se") {
        733
    } else if name.contains("Re") {
        1866
    } else {
        1900
    }
}

/// Interpolation times (ms) of the US male library (7): the hits' 200.
fn interp_of(name: &str) -> u32 {
    if name == "D_HIT" || name == "D_HITT" { 200 } else { 0 }
}

/// A stand-in entry: two keys (0 and `length`) on no nodes.
pub fn stand_in(name: &str, length: u32, interp: u32) -> Entry {
    Entry {
        animation: Animation {
            dims: [15.0, 50.0, 15.0],
            name: name.to_string(),
            compression: 0,
            interp_ms: interp,
            keys: vec![
                Key {
                    time: 0,
                    string: String::new(),
                },
                Key {
                    time: length.max(1),
                    string: String::new(),
                },
            ],
            channels: Vec::new(),
        },
        child: 0,
        dims: [15.0, 50.0, 15.0],
        translation: [0.0; 3],
    }
}

/// The library without a body model: entry 0 the body's own animation
/// (`baseAnim`, 4800 ms), then every name section 6 can build for the
/// classes 0 to 29 (lengths and blend times as measured, the rest by
/// kind), minus those the US libraries lack; the death animations (10);
/// for `female`, no `NS1`/`NS2`.
pub fn spec_library(female: bool) -> Arc<AnimList> {
    static LISTS: OnceLock<[Arc<AnimList>; 2]> = OnceLock::new();
    LISTS.get_or_init(|| [build_library(false), build_library(true)])[usize::from(female)].clone()
}

fn build_library(female: bool) -> Arc<AnimList> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: String| {
        if !missing(&n) && !names.iter().any(|m| m == &n) {
            names.push(n);
        }
    };
    const DIRS: [Direction; 9] = [
        Direction::None,
        Direction::F,
        Direction::B,
        Direction::L,
        Direction::R,
        Direction::FL,
        Direction::FR,
        Direction::BL,
        Direction::BR,
    ];
    for legs in 0..3 {
        for m in [Movement::Walk, Movement::Run, Movement::Sprint, Movement::Jump] {
            for d in DIRS {
                let (lower, crouch) = leg_names(legs, m, d);
                if let Some(l) = lower {
                    add(l);
                }
                add(crouch);
            }
        }
    }
    const POSTURES: [Posture; 9] = [
        Posture::Aim,
        Posture::PreFire,
        Posture::Fire,
        Posture::PostFire,
        Posture::Select,
        Posture::Reload,
        Posture::Hit,
        Posture::Hit2,
        Posture::AltFire,
    ];
    for class in 0..30 {
        for p in POSTURES {
            for d in DIRS {
                let (run, walk, sprint) = upper_names(class, p, d);
                add(run);
                walk.into_iter().for_each(&mut add);
                sprint.into_iter().for_each(&mut add);
            }
        }
    }
    for (n, _) in LENGTHS.iter().filter(|(n, _)| n.starts_with('D') || n.starts_with("NS")) {
        if !(female && n.starts_with("NS")) {
            add(n.to_string());
        }
    }
    for n in ["DC_1", "DCH", "DCH_1", "DCB", "DCB_1", "DCBH", "DCBH_1", "DHB_1", "DTB_1", "D_BO_B", "D_BO_L", "D_BO_R", "SD"] {
        add(n.to_string());
    }
    // The emotes the US male library has (9: not `Insult` or `Salute`;
    // their lengths aren't measured: the kind's default).
    for n in ["Surrender", "Insult_Dance"] {
        add(n.to_string());
    }
    let mut list = AnimList::default();
    list.offsets.push(Some(0));
    list.entries.push(stand_in(if female { "W_VST_NM" } else { "baseAnim" }, if female { 3333 } else { 4800 }, 0));
    for n in names {
        let (length, interp) = (length_of(&n), interp_of(&n));
        list.entries.push(stand_in(&n, length, interp));
    }
    Arc::new(list)
}
