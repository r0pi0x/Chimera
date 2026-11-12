//! The view model's animation state machine (specs/combat_arms/
//! view_models.md 7 and 8, its "Per-frame order"), as plain data: which
//! of the model's animations plays for each action, on a millisecond
//! tracker with the game's "done" and key rules (animation.md, "Playback").
//! One `ViewMachine` per draw (every draw starts from a fresh model). Each
//! rendered frame the caller runs `update` (the weapon update: polls
//! "done" as of the last advance, starts animations, hands over a shot a
//! key raised in the last advance), reads `shown` (the pose drawn this
//! frame) and then `advance`s by the frame's time (keys fire there).
//!
//! The tracker counts the frame's exact time times the rate (the spec's
//! test cases' arithmetic: 60 fps frames are 16.667 ms); a non-looping
//! animation is done once its time reaches L + 1 ms, a key fires in the
//! advance that takes the time past it (a key at 0 in the first one).
//!
//! What the caller does with the outputs is its own: the shots a key
//! raises (`Update::shot`) and the clip a finished reload fills
//! (`Update::reloaded`, `shell_in`) are the game's in the scenario tests
//! of the spec; in play the weapon frame (`weapons`, `inventory`) keeps
//! deciding shots and ammo, and these only drive the picture.

use std::sync::Arc;

use lithtech_formats::anim::{key_commands, weapon_rate};

/// `idle_wait`: the full idle's wait, uniform in this range (s).
pub const IDLE_WAIT: (f64, f64) = (5.0, 15.0);

/// Animation names looked up (lower case; matched without case).
pub mod names {
    pub const SELECT: &str = "select";
    pub const IDLE: &str = "idle_0";
    pub const PREFIRE: &str = "prefire";
    pub const FIRE: &str = "fire_0";
    pub const ALT_FIRE: &str = "alt_fire_0";
    pub const POSTFIRE: &str = "postfire";
    pub const DRY_FIRE: &str = "dryfire";
    pub const RELOAD: &str = "reload";
    /// The double barrel's "one barrel left" reload (7).
    pub const RELOAD_ONE: &str = "reload_1";
    /// The holster, when a file has one (none stock).
    pub const DESELECT: &str = "deselect";
    pub const PRE_RELOAD: &str = "prereload";
    pub const POST_RELOAD: &str = "postreload";
    pub const PRE_RUSH: &str = "prerush";
    pub const RUSH: &str = "rush";
    pub const POST_RUSH: &str = "postrush";
}

/// One animation of a view model: its length L and keys (ms, string;
/// with at most one key it never plays and is always done).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimInfo {
    pub name: String,
    pub length: u32,
    pub keys: Vec<(u32, String)>,
}

impl AnimInfo {
    /// A stand-in with keys at 0 and L, plus `strings` at their times.
    pub fn new(name: &str, length: u32, strings: &[(u32, &str)]) -> Self {
        let mut keys: Vec<(u32, String)> = vec![(0, String::new()), (length, String::new())];
        for (t, s) in strings {
            match keys.iter_mut().find(|k| k.0 == *t && k.1.is_empty()) {
                Some(k) => k.1 = s.to_string(),
                None => keys.push((*t, s.to_string())),
            }
        }
        keys.sort_by_key(|k| k.0);
        Self {
            name: name.to_string(),
            length,
            keys,
        }
    }
}

/// The weapon's class as the view model sees it (`inventory.md` classes,
/// with primary shotguns apart: they skip the pre-fire).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Primary,
    Shotgun,
    Secondary,
    Melee,
    Throw,
    Special,
}

impl Class {
    /// 7: `prefire` is skipped for secondary and melee classes and for
    /// primary shotguns.
    pub fn plays_prefire(self) -> bool {
        matches!(self, Class::Primary | Class::Throw | Class::Special)
    }
}

/// The animation rates (already summed with the attachments' and the
/// item's bonuses); any above 2.0 plays at 1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub fire: f32,
    pub reload: f32,
    pub draw: f32,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            fire: 1.0,
            reload: 1.0,
            draw: 1.0,
        }
    }
}

/// What the machine knows of the weapon drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setup {
    pub class: Class,
    /// `RapidFireCount`: shots per press (0: fully automatic). Above 0 the
    /// gun is semi-automatic for the post-fire cancel.
    pub per_press: u32,
    /// Reloads shell by shell (`PreReload`, `reload` per shell,
    /// `PostReload`).
    pub shells: bool,
    /// Clip size (0: no magazine, never reloads).
    pub clip_size: u32,
    pub rates: Rates,
    /// The double-barrel gun type: `reload_1` with rounds left (7).
    pub double_barrel: bool,
    /// The ammo's maximum (the projectile piece shows at a draw with a
    /// full total, 3).
    pub ammo_max: u32,
    /// `PVAttachFXName0..` effects, and `PVAttachFXAlwaysShow` (3).
    pub attach_fx: usize,
    pub attach_fx_always: bool,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            class: Class::Primary,
            per_press: 0,
            shells: false,
            clip_size: 0,
            rates: Rates::default(),
            double_barrel: false,
            ammo_max: 0,
            attach_fx: 0,
            attach_fx_always: false,
        }
    }
}

/// What the pieces and attached effects of the model show (3): the
/// `HiddenPiece` list hidden (total ammunition below 1, checked at the
/// draw and the end of each fire sequence), the `ProjectilePiece` shown,
/// each attached effect's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pieces {
    pub hidden_list: bool,
    pub projectile: bool,
    /// Per attached effect: shown now, hidden by a key.
    pub fx_shown: Vec<bool>,
    pub fx_hidden_by_key: Vec<bool>,
}

/// One weapon update's inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    /// Fire and alternate fire held.
    pub fire: bool,
    pub alt_fire: bool,
    /// A reload asked for this update (the key, or the game's start).
    pub reload: bool,
    pub clip: u32,
    /// Total ammo, the clip included.
    pub total: u32,
    pub sprinting: bool,
    /// Moving (a movement key or jump held): no full idle.
    pub moving: bool,
}

/// Which fire a shot came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireType {
    Primary,
    Alternate,
}

/// A key event (8) or the sound of entering pre- or post-fire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewKey {
    Fire,
    AltFire,
    /// `THROW_KEY`: the server's throw notice.
    Throw,
    /// `sound_key n` (1-3 reload, 4 select, 5 deselect, 10-14 misc).
    Sound(u32),
    /// `SHELL_CASING`.
    ShellCasing,
    /// Entering `prefire` / `postfire`: the weapon's `PreFireSnd` /
    /// `PostFireSnd`, if it has one.
    PreFireSound,
    PostFireSound,
}

/// What one weapon update did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Update {
    /// A shot (or knife hit) a key raised in the last advance, consumed now.
    pub shot: Option<FireType>,
    /// A magazine reload ended: the clip fills now.
    pub reloaded: bool,
    /// One shell of a shell reload went in.
    pub shell_in: bool,
    pub events: Vec<ViewKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireStage {
    /// The state is "firing", no animation started yet.
    Requested,
    Pre,
    Main,
    Post,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadStage {
    Magazine,
    Start,
    Shell,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SprintStage {
    In,
    Loop,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Drawing,
    Idle,
    Firing(FireStage),
    Reloading(ReloadStage),
    Sprint(SprintStage),
    /// `deselect` playing: the switch waits for it.
    Holstering,
}

/// The model's main tracker.
#[derive(Clone, Debug, Default)]
struct Tracker {
    anim: Option<usize>,
    /// ms, rate-scaled.
    t: f64,
    rate: f64,
    looping: bool,
    next_key: usize,
    /// After a non-reset switch, until the next advance: the previous
    /// animation at the key it was heading for (Quirk 1).
    pop: Option<(usize, f64)>,
}

/// The state machine of one drawn view model.
#[derive(Clone, Debug)]
pub struct ViewMachine {
    anims: Arc<Vec<AnimInfo>>,
    setup: Setup,
    pub stage: Stage,
    tracker: Tracker,
    /// The last fire type (alternate: `alt_fire_0`).
    alt: bool,
    /// A shot raised by a key in the last advance.
    pending: Option<FireType>,
    /// Shots of the current press, and whether fire was held last update.
    press_shots: u32,
    /// A reload request waiting for the fire sequence to end.
    reload_pending: bool,
    /// The automatic reload request (raised at the end of an update that
    /// is idle with an empty clip).
    auto_reload: bool,
    /// The weapon is mid-reload (its clip not yet filled): the empty clip
    /// is stale then, and asks for no second reload (`set_weapon_reloading`).
    weapon_reloading: bool,
    /// The clock (s) and when the full idle is due.
    clock: f64,
    idle_at: f64,
    rng: u32,
    /// The pieces and attached effects (3); the model visible (11).
    pub pieces: Pieces,
    visible: bool,
}

impl ViewMachine {
    /// A fresh model at the draw: `select` from 0 at the draw rate.
    pub fn draw(anims: Arc<Vec<AnimInfo>>, setup: Setup, seed: u32) -> Self {
        let mut m = Self {
            anims,
            setup,
            stage: Stage::Drawing,
            tracker: Tracker::default(),
            alt: false,
            pending: None,
            press_shots: 0,
            reload_pending: false,
            auto_reload: false,
            weapon_reloading: false,
            clock: 0.0,
            idle_at: f64::MAX,
            rng: seed | 1,
            pieces: Pieces {
                fx_shown: vec![false; setup.attach_fx],
                fx_hidden_by_key: vec![false; setup.attach_fx],
                ..Default::default()
            },
            visible: false,
        };
        m.start(names::SELECT, true, setup.rates.draw);
        m
    }

    /// The pieces at the draw (3): the hidden list by the total, the
    /// projectile piece shown with rounds in the clip or a full total.
    pub fn draw_ammo(&mut self, clip: u32, total: u32) {
        self.pieces.hidden_list = total < 1;
        self.pieces.projectile = clip > 0 || (self.setup.ammo_max > 0 && total >= self.setup.ammo_max);
    }

    /// A shot the weapon fired (3): the projectile piece hides, as on a
    /// shot a key raised (`Update::shot`).
    pub fn fired(&mut self) {
        self.pieces.projectile = false;
    }

    /// The model shown or hidden (the draw, a scope, 11): attached effects
    /// not hidden by a key show (all of them with `PVAttachFXAlwaysShow`),
    /// and all hide with the model.
    pub fn set_visible(&mut self, visible: bool) {
        if visible == self.visible {
            return;
        }
        self.visible = visible;
        for (i, shown) in self.pieces.fx_shown.iter_mut().enumerate() {
            *shown = visible && (self.setup.attach_fx_always || !self.pieces.fx_hidden_by_key[i]);
        }
    }

    /// Holster (7): `deselect` at rate 1.0 when the file has it (the
    /// switch waits for `holstered`); false without it (an instant switch).
    pub fn holster(&mut self) -> bool {
        if !self.start(names::DESELECT, false, 1.0) {
            return false;
        }
        self.stage = Stage::Holstering;
        true
    }

    /// The holster is over (or there was none).
    pub fn holstered(&self) -> bool {
        self.stage != Stage::Holstering || self.done()
    }

    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// Change the rates (attachments fitted while drawn); used from the
    /// next animation started.
    /// Whether the weapon is mid-reload: the animation can end a tick before
    /// the weapon fills the clip, and the stale empty clip must not raise
    /// the automatic reload again (a second reload animation).
    pub fn set_weapon_reloading(&mut self, reloading: bool) {
        self.weapon_reloading = reloading;
    }

    pub fn set_rates(&mut self, rates: Rates) {
        self.setup.rates = rates;
    }

    pub fn anims(&self) -> &[AnimInfo] {
        &self.anims
    }

    /// The first animation named `name` (case ignored).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.anims.iter().position(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// The animation playing (its index).
    pub fn current(&self) -> Option<usize> {
        self.tracker.anim
    }

    /// The name of the animation playing.
    pub fn playing(&self) -> Option<&str> {
        Some(self.anims.get(self.tracker.anim?)?.name.as_str())
    }

    /// Its time, ms.
    pub fn time(&self) -> f64 {
        self.tracker.t
    }

    /// The pose drawn this frame: (animation, time in ms); after a
    /// non-reset switch, the previous animation at its next key (Quirk 1).
    pub fn shown(&self) -> Option<(usize, f64)> {
        self.tracker.pop.or(self.tracker.anim.map(|a| (a, self.tracker.t)))
    }

    /// Whether the animation playing has ended (looping: never).
    pub fn done(&self) -> bool {
        let Some(info) = self.tracker.anim.and_then(|a| self.anims.get(a)) else {
            return true;
        };
        if info.keys.len() <= 1 {
            return true;
        }
        !self.tracker.looping && self.tracker.t >= info.length as f64 + 1.0 - 1e-6
    }

    fn roll(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f64 / (1u32 << 24) as f64
    }

    fn schedule_idle(&mut self) {
        let r = self.roll();
        self.idle_at = self.clock + IDLE_WAIT.0 + (IDLE_WAIT.1 - IDLE_WAIT.0) * r;
    }

    /// Start `name` (false if the model has none): reset from 0, or a
    /// non-reset switch (one-frame pop of the previous animation).
    fn start(&mut self, name: &str, reset: bool, rate: f32) -> bool {
        let Some(i) = self.find(name) else { return false };
        self.start_index(i, reset, rate, name.eq_ignore_ascii_case(names::RUSH));
        true
    }

    fn start_index(&mut self, i: usize, reset: bool, rate: f32, looping: bool) {
        let pop = if reset {
            None
        } else {
            self.tracker.anim.and_then(|prev| {
                let keys = &self.anims.get(prev)?.keys;
                let b = keys.iter().find(|k| k.0 as f64 >= self.tracker.t).or(keys.last())?;
                Some((prev, b.0 as f64))
            })
        };
        self.tracker = Tracker {
            anim: Some(i),
            t: 0.0,
            rate: weapon_rate(rate) as f64,
            looping,
            next_key: 0,
            pop,
        };
    }

    fn playing_is(&self, name: &str) -> bool {
        self.playing().is_some_and(|p| p.eq_ignore_ascii_case(name))
    }

    fn in_fire_sequence(&self) -> bool {
        [names::PREFIRE, names::FIRE, names::ALT_FIRE, names::POSTFIRE]
            .iter()
            .any(|n| self.playing_is(n))
    }

    /// The numbered full idles present (`idle_1`, `idle_2`, ...).
    fn full_idles(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for n in 1.. {
            match self.find(&format!("idle_{n}")) {
                Some(i) => out.push(i),
                None => break,
            }
        }
        out
    }

    /// Start a full idle (`reset`: from 0, else a switch with the
    /// one-frame pop); false without one.
    fn full_idle(&mut self, reset: bool) -> bool {
        let idles = self.full_idles();
        if idles.is_empty() {
            return false;
        }
        let pick = idles[((self.roll() * idles.len() as f64) as usize).min(idles.len() - 1)];
        self.start_index(pick, reset, 1.0, false);
        true
    }

    fn fire_anim(&self) -> &'static str {
        if self.alt { names::ALT_FIRE } else { names::FIRE }
    }

    fn sprint_in(&mut self) {
        self.stage = Stage::Sprint(SprintStage::In);
        if !self.start(names::PRE_RUSH, true, 1.0) && !self.start(names::RUSH, true, 1.0) {
            self.stage = Stage::Sprint(SprintStage::Loop);
        }
    }

    fn go_idle(&mut self) {
        self.stage = Stage::Idle;
        self.schedule_idle();
    }

    fn can_reload(&self, i: &Inputs) -> bool {
        self.setup.clip_size > 0 && i.clip < self.setup.clip_size && i.total > i.clip
    }

    fn start_reload(&mut self, i: &Inputs) {
        self.reload_pending = false;
        self.auto_reload = false;
        let rate = self.setup.rates.reload;
        if self.setup.shells {
            self.stage = Stage::Reloading(ReloadStage::Start);
            if !self.start(names::PRE_RELOAD, false, rate) {
                self.next_shell(i);
            }
        } else {
            self.stage = Stage::Reloading(ReloadStage::Magazine);
            let one = self.setup.double_barrel && i.clip > 0 && self.find(names::RELOAD_ONE).is_some();
            self.start(if one { names::RELOAD_ONE } else { names::RELOAD }, false, rate);
        }
        // The projectile piece shows when a reload starts with rounds to
        // load.
        if self.setup.clip_size > 0 && i.total > i.clip {
            self.pieces.projectile = true;
        }
    }

    /// The next stage of a shell reload: another shell or the end.
    fn next_shell(&mut self, i: &Inputs) {
        let rate = self.setup.rates.reload;
        if i.clip < self.setup.clip_size && i.total > i.clip {
            self.stage = Stage::Reloading(ReloadStage::Shell);
            self.start(names::RELOAD, false, rate);
        } else {
            self.stage = Stage::Reloading(ReloadStage::End);
            if !self.start(names::POST_RELOAD, false, rate) {
                self.go_idle();
            }
        }
    }

    /// One weapon update (Per-frame order 5.3).
    pub fn update(&mut self, i: &Inputs) -> Update {
        let mut out = Update {
            shot: self.pending.take(),
            ..Default::default()
        };
        if out.shot.is_some() {
            self.press_shots += 1;
            self.pieces.projectile = false;
        }
        let held = i.fire || (self.setup.class == Class::Melee && i.alt_fire);
        if !held {
            self.press_shots = 0;
        }
        // The clip-0 gate clears the request before the state machine.
        let request = held
            && i.clip > 0
            && (self.setup.per_press == 0 || self.press_shots < self.setup.per_press);
        let reload = i.reload || self.auto_reload;
        // A request while the reload already plays (the weapon's reload
        // starting a frame after the view model's own automatic one) is
        // that reload: queued, it would play again once this one ends
        // and before the weapon fills the clip.
        if i.reload && self.can_reload(i) && !matches!(self.stage, Stage::Reloading(_)) {
            self.reload_pending = true;
        }
        let done = self.done();
        let throw = self.setup.class == Class::Throw;
        let fire_rate = self.setup.rates.fire;
        // Sprinting cuts a knife swing and a double-barrel shot at once
        // (Corrections 3): their fire animations count as done.
        let cut = i.sprinting && (self.setup.class == Class::Melee || self.setup.double_barrel);
        match self.stage {
            Stage::Drawing => {
                if reload && self.can_reload(i) {
                    self.start_reload(i);
                } else if done {
                    if i.sprinting {
                        self.sprint_in();
                    } else if request {
                        self.alt = !i.fire;
                        self.stage = Stage::Firing(FireStage::Requested);
                    } else {
                        self.go_idle();
                    }
                }
            }
            Stage::Idle => {
                if (reload || self.reload_pending) && self.can_reload(i) {
                    self.start_reload(i);
                } else if i.sprinting {
                    self.sprint_in();
                } else if request {
                    self.alt = !i.fire;
                    self.stage = Stage::Firing(FireStage::Requested);
                } else if done {
                    // The subtle idle restarts once done; a full idle
                    // when its wait has passed and the player stands still
                    // (the wait is drawn again each time it is checked).
                    let due = self.clock >= self.idle_at;
                    if due {
                        self.schedule_idle();
                    }
                    if !(due && !i.moving && self.full_idle(false)) {
                        self.start(names::IDLE, true, 1.0);
                    }
                }
            }
            Stage::Firing(FireStage::Requested) => {
                // Between the request and the pin pull the throw class is
                // not yet holding a fire animation: sprint drops the
                // request (Corrections 4).
                if i.sprinting {
                    self.sprint_in();
                } else if self.setup.class.plays_prefire() && self.start(names::PREFIRE, true, fire_rate) {
                    self.stage = Stage::Firing(FireStage::Pre);
                    out.events.push(ViewKey::PreFireSound);
                } else {
                    self.stage = Stage::Firing(FireStage::Main);
                    self.start(self.fire_anim(), true, fire_rate);
                }
            }
            Stage::Firing(FireStage::Pre) => {
                if done && !(throw && i.sprinting) {
                    self.stage = Stage::Firing(FireStage::Main);
                    if !self.start(self.fire_anim(), true, fire_rate) {
                        self.after_fire(i, &mut out);
                    }
                }
            }
            Stage::Firing(FireStage::Main) => {
                if (done || cut) && !(throw && i.sprinting) {
                    if i.sprinting {
                        self.sprint_in();
                    } else if request && i.total > 0 && !self.reload_pending {
                        self.alt = !i.fire;
                        self.start(self.fire_anim(), true, fire_rate);
                    } else {
                        self.after_fire(i, &mut out);
                    }
                }
            }
            Stage::Firing(FireStage::Post) => {
                if (done || cut) && i.sprinting && !throw {
                    self.sprint_in();
                } else if request && i.total > 0 && !self.reload_pending && !i.sprinting {
                    // A new press refires on the update that sees `postfire`
                    // done (a semi-automatic gun at once), with no idle
                    // between: the whole sequence again, `prefire` first
                    // where the weapon plays one (Corrections 1, 2).
                    let semi = self.setup.per_press > 0 && self.press_shots == 0;
                    if semi || done {
                        self.alt = !i.fire;
                        self.restart_sequence(&mut out);
                    }
                } else if done && !(throw && i.sprinting) {
                    if i.sprinting {
                        self.sprint_in();
                    } else {
                        self.go_idle();
                    }
                }
            }
            Stage::Reloading(r) => {
                if r == ReloadStage::Shell && request {
                    // Fire stops a shell reload at once.
                    self.alt = !i.fire;
                    self.stage = Stage::Firing(FireStage::Requested);
                } else if done {
                    match r {
                        ReloadStage::Magazine => {
                            out.reloaded = true;
                            self.pieces.projectile = true;
                            if i.sprinting {
                                self.sprint_in();
                            } else {
                                self.go_idle();
                            }
                        }
                        ReloadStage::Start => self.next_shell(i),
                        ReloadStage::Shell => {
                            out.shell_in = true;
                            // The caller's clip is one up now.
                            let next = Inputs {
                                clip: i.clip + 1,
                                ..*i
                            };
                            self.next_shell(&next);
                        }
                        ReloadStage::End => {
                            if i.sprinting {
                                self.sprint_in();
                            } else {
                                self.go_idle();
                            }
                        }
                    }
                }
            }
            Stage::Holstering => {}
            Stage::Sprint(s) => match s {
                SprintStage::In | SprintStage::Loop if !i.sprinting => {
                    self.stage = Stage::Sprint(SprintStage::Out);
                    if !self.start(names::POST_RUSH, true, 1.0) {
                        self.stage = Stage::Idle;
                        self.full_idle(false);
                    }
                }
                SprintStage::In if done => {
                    self.stage = Stage::Sprint(SprintStage::Loop);
                    self.start(names::RUSH, true, 1.0);
                }
                SprintStage::Out if i.sprinting => self.sprint_in(),
                SprintStage::Out if done => {
                    // Leaving sprint: a full idle from 0 (without one nothing
                    // starts now; the subtle idle follows on the next
                    // update), the wait not rescheduled (Corrections 5).
                    self.stage = Stage::Idle;
                    self.full_idle(true);
                }
                _ => {}
            },
        }
        // Step 8: the automatic reload request, for the next update.
        if self.stage == Stage::Idle && self.setup.clip_size > 0 && i.clip == 0 && i.total > 0 && !self.weapon_reloading {
            self.auto_reload = true;
        }
        out
    }

    /// The fire sequence again from its start: `prefire` (reset) when the
    /// weapon plays one, else the fire animation.
    fn restart_sequence(&mut self, out: &mut Update) {
        let rate = self.setup.rates.fire;
        if self.setup.class.plays_prefire() && self.start(names::PREFIRE, true, rate) {
            self.stage = Stage::Firing(FireStage::Pre);
            out.events.push(ViewKey::PreFireSound);
        } else {
            self.stage = Stage::Firing(FireStage::Main);
            self.start(self.fire_anim(), true, rate);
        }
    }

    /// The main fire animation ended without a repeat: post-fire if the
    /// model has one, else idle.
    fn after_fire(&mut self, i: &Inputs, out: &mut Update) {
        // The end of a fire sequence checks the hidden pieces (3).
        self.pieces.hidden_list = i.total < 1;
        if self.start(names::POSTFIRE, true, self.setup.rates.fire) {
            self.stage = Stage::Firing(FireStage::Post);
            out.events.push(ViewKey::PostFireSound);
        } else {
            self.go_idle();
        }
    }

    /// The engine's advance by the frame's time `dt` (s): the tracker
    /// moves on and fires its keys (a shot key raises a shot for the next
    /// update).
    pub fn advance(&mut self, dt: f64) -> Vec<ViewKey> {
        self.clock += dt;
        self.tracker.pop = None;
        let mut out = Vec::new();
        let Some(a) = self.tracker.anim else { return out };
        let anims = self.anims.clone();
        let Some(info) = anims.get(a) else { return out };
        if info.keys.len() <= 1 {
            self.tracker.t = 0.0;
            return out;
        }
        let length = info.length as f64;
        self.tracker.t += dt * 1000.0 * self.tracker.rate;
        if !self.tracker.looping {
            self.tracker.t = self.tracker.t.min(length + 1.0);
        }
        while let Some((kt, s)) = info.keys.get(self.tracker.next_key) {
            if (*kt as f64) >= self.tracker.t - 1e-9 {
                break;
            }
            self.tracker.next_key += 1;
            for cmd in key_commands(s) {
                if let Some(k) = self.key(&cmd) {
                    out.push(k);
                }
            }
        }
        if self.tracker.looping && self.tracker.next_key >= info.keys.len() {
            self.tracker.t = if length > 0.0 { self.tracker.t % length } else { 0.0 };
            self.tracker.next_key = 0;
        }
        out
    }

    /// One key command (8).
    fn key(&mut self, cmd: &[String]) -> Option<ViewKey> {
        let word = cmd.first()?.to_ascii_lowercase();
        let firing = matches!(self.stage, Stage::Firing(_)) && self.in_fire_sequence();
        match word.as_str() {
            "fire_key" | "alt_fire_key" => {
                let alt = word == "alt_fire_key";
                if firing {
                    self.pending = Some(if alt { FireType::Alternate } else { FireType::Primary });
                }
                Some(if alt { ViewKey::AltFire } else { ViewKey::Fire })
            }
            "throw_key" => Some(ViewKey::Throw),
            "sound_key" => cmd.get(1)?.parse().ok().map(ViewKey::Sound),
            "shell_casing" => Some(ViewKey::ShellCasing),
            "hide_pvattachfx" | "show_pvattachfx" => {
                let hide = word == "hide_pvattachfx";
                if self.visible && !(hide && self.setup.attach_fx_always) {
                    for i in cmd.iter().skip(1).filter_map(|n| n.parse::<usize>().ok()) {
                        if i < self.pieces.fx_shown.len() {
                            self.pieces.fx_hidden_by_key[i] = hide;
                            self.pieces.fx_shown[i] = !hide;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
