//! One Halo weapon's update, one 30 Hz tick at a time, in the spec's
//! per-tick order (specs/halo_ce/weapons.md "Per-tick order", step 4, and
//! sections 1, 2, 4 to 9, 11 and 12): `HaloGun` is the runtime state of a
//! weapon with one trigger and at most one magazine (the four M2 weapons'
//! shape), `HaloGun::tick` runs one update from the holder's `Inputs` and
//! returns what happened (`GunEvent`: the shots with their directions,
//! clicks, reloads, overheats). Pure: no ECS, so the spec's timelines are
//! tested tick by tick (`super::systems` runs it in the game).
//!
//! The order inside a tick: the weapon's own animation (recoil, chamber)
//! steps; heat (overheat, cooling, recovery); the state timer (the draw)
//! counts down; the trigger reads down only when held, the holder not
//! busy and the timer at 0; the reload input; the magazine (reload end,
//! the shotgun's next shell, chambering); the trigger (the automatic
//! reload check, then firing when it may fire again, else the idle
//! counter), then the rate of fire and the error ramp, all in single
//! precision with the tag's per-tick rates.
//!
//! Known shortcuts: one trigger and one magazine (the plasma pistol's
//! second trigger, charging, spewing, overloading and tracking are not
//! here: none of the four weapons has them); no under-water check (the
//! level's water is not known to the simulation yet); ejection port,
//! illumination and the export functions are not kept (presentation
//! reads the events and `HaloGun` itself).

use bevy::math::Vec3;
use serde::{Deserialize, Serialize};

use super::{
    params::{Gun, trigger_flags, weapon_flags},
    random::{HaloRng, cone},
};

/// The idle counter's cap (a signed 8-bit count).
pub const IDLE_TICKS_CAP: i32 = 127;

/// The weapon's state (weapons.md 1), numbered as the spec lists them:
/// a state is entered unforced only from idle, or from a recoil to a
/// state not numbered lower.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WeaponMode {
    #[default]
    Idle,
    Recoil,
    Chamber,
    Reload,
    Ready,
    PutAway,
}

/// The trigger's state (weapons.md 4; the four weapons use these).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMode {
    #[default]
    Idle,
    /// A pull that did not fire: until released.
    Locked,
    /// After a draw or a put-away: idle once its timer is 0.
    Uninitialised,
}

/// The magazine's state (weapons.md 11).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MagazineMode {
    #[default]
    Idle,
    Reloading,
    Unchambered,
    Chambering,
}

/// A weapon's runtime state (weapons.md 1). `loaded` and `reserve` are
/// the magazine's rounds (also the engine's `weapon::Magazine`, which the
/// game keeps equal).
#[derive(bevy::prelude::Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HaloGun {
    pub heat: f32,
    pub age: f32,
    pub overheated: bool,
    /// The heat will be below the recovery threshold within a tick (the
    /// first-person exit animation).
    pub overheat_exit: bool,
    pub needs_reload: bool,
    pub mode: WeaponMode,
    /// The weapon's own animation frame in `mode` (recoil, chamber).
    pub mode_frame: i32,
    /// Ticks before the triggers read (the draw).
    pub state_timer: i32,
    pub overcharged: f32,
    /// The game-state generator's seed (`random::HaloRng`).
    pub rng: u32,
    // The trigger.
    pub idle_ticks: i32,
    pub trigger: TriggerMode,
    pub trigger_timer: i32,
    pub released_since_shot: bool,
    pub rate_of_fire: f32,
    pub error: f32,
    pub effect_index: u8,
    pub effect_remaining: i32,
    pub effect_used: u32,
    pub tracer_count: i32,
    // The magazine.
    pub magazine: MagazineMode,
    pub magazine_timer: i32,
    /// The reload in progress started the shotgun's loop (its later
    /// shells are timed by `reload-empty`, the first by `enter`).
    pub loading_shells: bool,
    pub loaded: i16,
    pub reserve: i16,
    pub recharge: i32,
    /// The zoom level (-1: none; weapons.md 12).
    pub zoom: i16,
    pub zoom_held: bool,
    /// In its holder's hand (a change readies or puts it away).
    pub in_hand: bool,
}

/// What the holder passes the weapon for one tick (weapons.md 4,
/// "Inputs").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub fire: bool,
    pub reload: bool,
    pub zoom: bool,
    /// The holder's animation is busy or a player melee runs.
    pub busy: bool,
    /// In a holder's hand (an inventory weapon gets no inputs and never
    /// reloads on its own).
    pub held: bool,
    /// A switch is pending.
    pub switching: bool,
    /// The aiming vector (unit).
    pub aim: Vec3,
}

/// What one tick did, for the game and its presentation.
#[derive(Clone, Debug, PartialEq)]
pub enum GunEvent {
    /// A round left (or misfired): the projectiles' directions and
    /// whether each is a tracer; `last_round`: the magazine is now empty;
    /// `effect`: the firing effect entry (the plasma rifle's barrels).
    Fired { misfired: bool, last_round: bool, effect: u8, projectiles: Vec<(Vec3, bool)> },
    /// A pull with too few rounds (the empty effect, once a pull).
    Empty,
    /// A reload began; `shell`: one shotgun shell, `first`: the loop's
    /// first (its `enter`).
    ReloadStarted { shell: bool, first: bool },
    /// Rounds went into the magazine (a reload's end, each shell).
    RoundsIn { shell: bool },
    /// The weapon chambers a round (the shotgun's pump).
    Chambering,
    Overheated,
    OverheatCleared,
    /// The zoom level changed (-1: none).
    Zoom(i16),
}

impl HaloGun {
    /// A new weapon (weapons.md 1): rounds by the magazine, the idle
    /// counter at its cap.
    pub fn new(gun: &Gun) -> Self {
        let (loaded, reserve) = gun.magazine.map_or((0, 0), |m| m.new_rounds());
        HaloGun { idle_ticks: IDLE_TICKS_CAP, loaded, reserve, zoom: -1, ..Default::default() }
    }

    /// The generator.
    fn rng(&self) -> HaloRng {
        HaloRng(self.rng)
    }

    /// Enter a weapon state (weapons.md 1): unforced only from idle or a
    /// recoil to a state not lower; recorded only when the weapon's own
    /// graph has the state's animation.
    fn enter(&mut self, gun: &Gun, mode: WeaponMode, forced: bool) {
        if !forced && !(self.mode == WeaponMode::Idle || (self.mode == WeaponMode::Recoil && mode >= self.mode)) {
            return;
        }
        let has = match mode {
            WeaponMode::Idle => true,
            WeaponMode::Recoil => gun.animations.world_recoil.is_some(),
            WeaponMode::Chamber => gun.animations.world_chamber.is_some(),
            _ => false,
        };
        if has {
            self.mode = mode;
            self.mode_frame = 0;
        }
    }

    /// Drawn into the hand (weapons.md 2): the triggers uninitialised, a
    /// reload settled, the state timer the ready animation's frames.
    pub fn ready(&mut self, gun: &Gun) -> Option<GunEvent> {
        let rounds = self.settle_reload(gun);
        self.trigger = TriggerMode::Uninitialised;
        self.trigger_timer = 0;
        self.enter(gun, WeaponMode::Ready, true);
        self.state_timer = gun.animations.ready;
        self.in_hand = true;
        rounds
    }

    /// Put away (weapons.md 2): a reload settled, the triggers
    /// uninitialised, the zoom left.
    pub fn put_away(&mut self, gun: &Gun) -> Option<GunEvent> {
        let rounds = self.settle_reload(gun);
        self.trigger = TriggerMode::Uninitialised;
        self.trigger_timer = 0;
        self.zoom = -1;
        self.in_hand = false;
        rounds
    }

    /// A reload interrupted (a switch, a melee, a grenade; weapons.md 11
    /// "Interruptions"): finished at once when more than half done
    /// (2 T < F), else lost; the magazine idle, the triggers
    /// uninitialised.
    pub fn settle_reload(&mut self, gun: &Gun) -> Option<GunEvent> {
        let mut out = None;
        if self.magazine == MagazineMode::Reloading {
            let f = gun.animations.reload_empty.unwrap_or(0);
            if 2 * self.magazine_timer < f {
                self.add_rounds(gun);
                out = Some(GunEvent::RoundsIn { shell: self.loading_shells });
            }
            self.magazine = MagazineMode::Idle;
            self.magazine_timer = 0;
            self.loading_shells = false;
            self.trigger = TriggerMode::Uninitialised;
            self.trigger_timer = 0;
        } else if self.magazine != MagazineMode::Idle {
            self.magazine = MagazineMode::Idle;
            self.magazine_timer = 0;
        }
        out
    }

    /// One tick (weapons.md "Per-tick order", 4.1 to 4.9).
    pub fn tick(&mut self, gun: &Gun, inputs: &Inputs) -> Vec<GunEvent> {
        let mut out = Vec::new();
        // Zoom (step 1: the press steps the level; not while magazine 0
        // reloads).
        let zoom_press = inputs.held && inputs.zoom && !self.zoom_held;
        self.zoom_held = inputs.held && inputs.zoom;
        if zoom_press && gun.zoom_levels > 0 && self.magazine != MagazineMode::Reloading {
            self.zoom = if self.zoom + 1 >= gun.zoom_levels { -1 } else { self.zoom + 1 };
            out.push(GunEvent::Zoom(self.zoom));
        }
        // 4.1 The weapon's own animation.
        if matches!(self.mode, WeaponMode::Recoil | WeaponMode::Chamber) {
            let frames = match self.mode {
                WeaponMode::Recoil => gun.animations.world_recoil,
                _ => gun.animations.world_chamber,
            }
            .unwrap_or(1);
            self.mode_frame += 1;
            if self.mode_frame + 1 >= frames {
                self.mode = WeaponMode::Idle;
                self.mode_frame = 0;
            }
        }
        // 4.4 Heat.
        self.heat_step(gun, &mut out);
        // 4.5
        self.overcharged = 0.0;
        if self.state_timer > 0 {
            self.state_timer -= 1;
        }
        // 4.6 The trigger reads down.
        let down = inputs.held && inputs.fire && !inputs.busy && self.state_timer == 0;
        // 4.7 The reload input.
        if inputs.held && inputs.reload {
            self.needs_reload = true;
        }
        if self.needs_reload {
            self.try_reload(gun, false, &mut out);
        }
        // 4.8 The magazine.
        self.magazine_step(gun, inputs, &mut out);
        // 4.9 The trigger.
        self.trigger_step(gun, inputs, down, &mut out);
        out
    }

    fn heat_step(&mut self, gun: &Gun, out: &mut Vec<GunEvent>) {
        if self.heat <= 0.0 {
            return;
        }
        let h = &gun.heat;
        if self.heat >= h.overheated_threshold && !self.overheated {
            self.overheated = true;
            out.push(GunEvent::Overheated);
        }
        if self.overcharged == 0.0 {
            let mut loss = h.loss_per_second * (1.0 / 30.0);
            if gun.age.heat_recovery_penalty > 0.0 {
                loss *= 1.0 - self.age * gun.age.heat_recovery_penalty;
            }
            self.heat = (self.heat - loss).max(0.0);
            if self.overheated && loss > 0.0 && (self.heat - h.recovery_threshold) / loss <= 1.0 {
                self.overheat_exit = true;
            }
        }
        if self.overheated && self.heat < h.recovery_threshold {
            self.overheated = false;
            self.overheat_exit = false;
            out.push(GunEvent::OverheatCleared);
        }
    }

    /// Start a reload of the magazine (weapons.md 11): only from idle or
    /// unchambered, the trigger and the weapon idle; with reserve and
    /// room. `looped`: the shotgun's next shell.
    fn try_reload(&mut self, gun: &Gun, looped: bool, out: &mut Vec<GunEvent>) {
        let Some(m) = gun.magazine else {
            self.needs_reload = false;
            return;
        };
        if !matches!(self.magazine, MagazineMode::Idle | MagazineMode::Unchambered)
            || self.trigger != TriggerMode::Idle
            || self.mode != WeaponMode::Idle
        {
            return;
        }
        self.needs_reload = false;
        if self.reserve <= 0 || self.loaded >= m.rounds_loaded_maximum {
            return;
        }
        let Some(empty) = gun.animations.reload_empty else { return };
        let shells = gun.weapon_type == SHOTGUN;
        let first = shells && !looped;
        self.magazine = MagazineMode::Reloading;
        self.magazine_timer = match gun.animations.enter {
            Some(enter) if first => enter,
            _ => empty,
        };
        self.loading_shells = shells;
        self.enter(gun, WeaponMode::Reload, false);
        // The first-person reload unzooms.
        if self.zoom >= 0 {
            self.zoom = -1;
            out.push(GunEvent::Zoom(-1));
        }
        out.push(GunEvent::ReloadStarted { shell: shells, first });
    }

    /// The rounds of one reload moved in.
    fn add_rounds(&mut self, gun: &Gun) {
        let Some(m) = gun.magazine else { return };
        if m.wastes_rounds() {
            self.loaded = 0;
        }
        let n = m.rounds_reloaded.min(self.reserve);
        let loaded = (self.loaded + n).min(m.rounds_loaded_maximum);
        self.reserve -= loaded - self.loaded;
        self.loaded = loaded;
    }

    fn magazine_step(&mut self, gun: &Gun, inputs: &Inputs, out: &mut Vec<GunEvent>) {
        let Some(m) = gun.magazine else { return };
        // Recharge.
        if m.rounds_recharged > 0 && self.loaded < m.rounds_loaded_maximum {
            let r = m.rounds_recharged as i32;
            self.loaded += (r / 30) as i16;
            self.recharge += r % 30;
            if self.recharge >= 30 {
                self.loaded += 1;
                self.recharge -= 30;
            }
            self.loaded = self.loaded.min(m.rounds_loaded_maximum);
        }
        if self.magazine_timer > 0 {
            self.magazine_timer -= 1;
        }
        match self.magazine {
            MagazineMode::Reloading if self.magazine_timer - 1 <= 0 => {
                self.add_rounds(gun);
                self.magazine = MagazineMode::Unchambered;
                self.magazine_timer = 0;
                let shell = self.loading_shells;
                out.push(GunEvent::RoundsIn { shell });
                self.loading_shells = false;
                // The shotgun's loop: the next shell at once unless a
                // trigger is pressed or a switch pending.
                if self.reserve > 0
                    && self.loaded < m.rounds_loaded_maximum
                    && !m.wastes_rounds()
                    && !(inputs.held && inputs.fire)
                    && !inputs.switching
                {
                    self.try_reload(gun, true, out);
                }
            }
            MagazineMode::Unchambered => {
                if self.trigger == TriggerMode::Idle && self.mode == WeaponMode::Idle {
                    self.enter(gun, WeaponMode::Chamber, false);
                    self.magazine = MagazineMode::Chambering;
                    self.magazine_timer = m.chamber_ticks();
                    out.push(GunEvent::Chambering);
                }
            }
            MagazineMode::Chambering if self.magazine_timer == 0 => {
                self.magazine = MagazineMode::Idle;
            }
            _ => {}
        }
    }

    fn trigger_step(&mut self, gun: &Gun, inputs: &Inputs, down: bool, out: &mut Vec<GunEvent>) {
        let t = &gun.trigger;
        if self.trigger_timer > 0 {
            self.trigger_timer -= 1;
        }
        if !down {
            self.released_since_shot = true;
        }
        match self.trigger {
            TriggerMode::Idle => {
                // The automatic reload check (a held weapon whose holder
                // is not busy).
                if inputs.held
                    && !inputs.busy
                    && let Some(_) = gun.magazine
                    && t.magazine >= 0
                {
                    let partial = t.has(trigger_flags::CAN_FIRE_WITH_PARTIAL_AMMO);
                    if (self.loaded < t.rounds_per_shot && !partial)
                        || self.loaded < t.minimum_rounds_loaded
                        || self.loaded == 0
                    {
                        self.try_reload(gun, false, out);
                    }
                }
                let semi_wait = t.has(trigger_flags::DOES_NOT_REPEAT_AUTOMATICALLY) && !self.released_since_shot;
                let required = t.required_ticks(self.rate_of_fire, self.age, gun.age.rate_of_fire_penalty);
                if down && !semi_wait && (self.idle_ticks + 1) as f32 >= required {
                    // Begin firing: refused while the magazine is busy or
                    // the weapon overheated (the counter stays).
                    if !(t.magazine >= 0 && gun.magazine.is_some() && self.magazine != MagazineMode::Idle) && !self.overheated {
                        self.fire(gun, inputs, out);
                    }
                } else {
                    self.idle_ticks = (self.idle_ticks + 1).min(IDLE_TICKS_CAP);
                }
            }
            TriggerMode::Locked => {
                if !down {
                    self.trigger = TriggerMode::Idle;
                }
            }
            TriggerMode::Uninitialised => {
                if self.trigger_timer == 0 {
                    self.trigger = TriggerMode::Idle;
                }
            }
        }
        // The ramps.
        if down {
            self.rate_of_fire = (self.rate_of_fire + t.rate_of_fire_acceleration).min(1.0);
            self.error = (self.error + t.error_acceleration).min(1.0);
        } else {
            self.rate_of_fire = (self.rate_of_fire - t.rate_of_fire_deceleration).max(0.0);
            self.error = (self.error - t.error_deceleration).max(0.0);
        }
    }

    /// A trigger fires (weapons.md 5 and 6).
    fn fire(&mut self, gun: &Gun, inputs: &Inputs, out: &mut Vec<GunEvent>) {
        let t = &gun.trigger;
        let mut rng = self.rng();
        // 1. Ammunition.
        let mut fired = true;
        if t.magazine >= 0
            && let Some(m) = gun.magazine
        {
            let partial = t.has(trigger_flags::CAN_FIRE_WITH_PARTIAL_AMMO);
            let dead = gun.has(weapon_flags::CANNOT_FIRE_AT_MAXIMUM_AGE) && self.age >= 1.0;
            fired = (self.loaded >= t.rounds_per_shot || partial)
                && !dead
                && (self.loaded >= t.minimum_rounds_loaded || !self.released_since_shot);
            if fired {
                self.loaded -= t.rounds_per_shot;
                if self.loaded <= 0 {
                    self.loaded = 0;
                } else if m.chambers_every_round() {
                    self.magazine = MagazineMode::Unchambered;
                }
            }
        } else if gun.has(weapon_flags::CANNOT_FIRE_AT_MAXIMUM_AGE) && self.age >= 1.0 {
            fired = false;
        }
        // 2. The firing effect entry (every shot count 1 - 1: one draw).
        let entries = t.firing_effects.max(1) as u32;
        if t.firing_effects > 0 && self.effect_remaining <= 0 {
            let start = self.effect_index as u32;
            let mut index = start;
            loop {
                index = (index + 1) % entries;
                let all = (1u32 << entries) - 1;
                if self.effect_used & all == all {
                    self.effect_used = 0;
                }
                if self.effect_used & (1 << index) != 0 {
                    if index == start {
                        break;
                    }
                    continue;
                }
                self.effect_used |= 1 << index;
                self.effect_remaining = rng.int_range(1, 1);
                if self.effect_remaining > 0 || index == start {
                    break;
                }
            }
            self.effect_index = index as u8;
        }
        self.effect_remaining -= 1;
        // 3. Misfire.
        let a = &gun.age;
        let mut misfired = false;
        if fired && a.misfire_start > 0.0 && a.misfire_start < 1.0 && self.age > a.misfire_start {
            let chance = (self.age - a.misfire_start) * a.misfire_chance / (1.0 - a.misfire_start);
            misfired = rng.real() < chance;
        }
        if fired {
            self.heat = (self.heat + t.heat_per_round).min(1.0);
            self.age = (self.age + t.age_per_round).min(1.0);
            self.enter(gun, WeaponMode::Recoil, false);
            let mut projectiles = Vec::new();
            if !misfired {
                // The error left by the previous tick (weapons.md 7).
                let theta_max = t.theta_max(self.error);
                let straight = t.has(trigger_flags::USE_ERROR_WHEN_UNZOOMED) && self.zoom >= 0;
                let mut first = None;
                for _ in 0..t.projectiles_per_shot.max(0) {
                    let mut d = if straight { inputs.aim } else { cone(&mut rng, inputs.aim, t.minimum_error, theta_max) };
                    if t.has(trigger_flags::PROJECTILES_HAVE_IDENTICAL_ERROR) {
                        d = *first.get_or_insert(d);
                    }
                    let tracer = if self.rate_of_fire == 0.0 || self.tracer_count >= t.rounds_between_tracers as i32 {
                        self.tracer_count = 0;
                        true
                    } else {
                        self.tracer_count += 1;
                        false
                    };
                    projectiles.push((d, tracer));
                }
            }
            // 6. Detonation (no multiplayer weapon reaches it).
            if self.heat > gun.heat.detonation_threshold {
                let _ = rng.real() < gun.heat.detonation_fraction;
            }
            out.push(GunEvent::Fired {
                misfired,
                last_round: gun.magazine.is_some_and(|m| m.rounds_loaded_maximum > 0) && self.loaded == 0,
                effect: self.effect_index,
                projectiles,
            });
            // 7. Recover.
            self.idle_ticks = 0;
            self.trigger = TriggerMode::Idle;
        } else {
            out.push(GunEvent::Empty);
            self.trigger = TriggerMode::Locked;
        }
        self.released_since_shot = false;
        self.rng = rng.0;
    }

    /// The error angle limit the next shot would use.
    pub fn theta_max(&self, gun: &Gun) -> f32 {
        gun.trigger.theta_max(self.error)
    }

    /// The battery left as the HUD shows it (1 - age).
    pub fn battery(&self) -> f32 {
        1.0 - self.age
    }
}

/// The shotgun's weapon type (its shell-by-shell loop).
pub const SHOTGUN: u16 = 1;
