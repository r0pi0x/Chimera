//! HL2:DM's HUD as one state machine (specs/hl2dm/hud.md, H8): what it
//! remembers between frames, the named HUD events it starts when the game's
//! numbers change (`events`: for tests and the log), and the frame it draws
//! (`Frame`, a list of boxes, text and sprites in window pixels).
//!
//! The panels' look is data: the layout, scheme and animation script are
//! the install's (`game::hud`), an event's flashes and glows are
//! `scripts/hudanimations.txt`'s (`source_engine::hud::anim`); the code
//! here decides only which event a change starts and what each variable
//! draws. The Bevy side (`systems`) fills `Inputs` from the simulation each
//! frame and draws the `Frame`.
//!
//! Per frame, in the spec's order: `think` (the elements start events on
//! changes), then the animations update (`Animator::update`), then
//! `frame` paints back to front: damage indicator, numeric panels, aux
//! power, weapon selection, pickup history, death notices, crosshair,
//! quick info, suit zoom, target ID, team line.

use bevy::prelude::*;

use super::{
    aux_power::Aux,
    damage::Damage,
    frame::Frame,
    history::History,
    look::Look,
    notices::Notices,
    numeric::Numeric,
    quick::{Quick, ZoomView},
    selection::Selection,
};
use source_engine::hud::anim::{Animator, Script, Value};

/// The weapon in hand, as the HUD reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponNow {
    /// Its script class (`weapon_smg1`), the key of its icons.
    pub class: String,
    /// It has a primary ammo type (the crowbar, stunstick, gravity gun and
    /// S.L.A.M. have none: the ammo panel hides).
    pub uses_primary: bool,
    /// Rounds in the clip; None: no clip (the frag, the RPG count the
    /// carried ammo).
    pub clip: Option<i32>,
    /// The clip's size (the low-ammo warning's base).
    pub max_clip: i32,
    /// The carried ammo of the primary type.
    pub reserve: i32,
    /// The secondary ammo type and the count carried.
    pub secondary: Option<(String, u32)>,
}

impl WeaponNow {
    /// What the ammo panel's digits show: the clip, else the carried ammo.
    pub fn ammo_value(&self) -> i32 {
        self.clip.unwrap_or(self.reserve)
    }
}

/// A weapon the player owns, for the selection.
#[derive(Clone, Debug, PartialEq)]
pub struct Owned {
    pub entity: Entity,
    pub class: String,
    /// 0-based bucket and position in it.
    pub bucket: u8,
    pub position: u8,
    /// It can be selected (it has ammo, or needs none).
    pub selectable: bool,
    /// The `#token` of its print name.
    pub print_name: String,
}

/// Everything the HUD reads from the game in one frame.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub alive: bool,
    /// Health in points.
    pub health: f32,
    /// Armour in points.
    pub armour: f32,
    pub weapon: Option<WeaponNow>,
    pub owned: Vec<Owned>,
    /// The weapon in hand.
    pub active: Option<Entity>,
    /// Aux power (0 to 100) and the devices using it.
    pub aux: f32,
    pub sprinting: bool,
    pub breathing: bool,
    /// The suit zoom is on.
    pub zoomed: bool,
    /// The player's current field of view (degrees).
    pub fov: f32,
    pub teamplay: bool,
    /// HL2:DM's team number (`game::rules::COMBINE`...).
    pub team: u8,
    /// The server's hide bits: health (observers), weapon selection
    /// (carrying a prop), crosshair, miscellaneous.
    pub hide_health: bool,
    pub hide_selection: bool,
    pub hide_crosshair: bool,
    pub hide_misc: bool,
    /// `crosshair` and `hud_quickinfo`.
    pub crosshair: bool,
    pub quickinfo: bool,
    /// `hud_showemptyweaponslots` is 0: the selection skips empty boxes.
    pub hide_empty_slots: bool,
}

/// A suit device count, from the inputs.
pub fn devices(inp: &Inputs) -> u8 {
    inp.sprinting as u8 + inp.breathing as u8
}

/// The HUD.
#[derive(Clone, Debug)]
pub struct Hl2Hud {
    pub anim: Animator,
    pub numeric: Numeric,
    pub aux: Aux,
    pub damage: Damage,
    pub quick: Quick,
    pub zoom: ZoomView,
    pub selection: Selection,
    pub history: History,
    pub notices: Notices,
    /// Client time of the last `think`.
    pub now: f64,
    /// The events started, newest last (the last 64): what the tests and
    /// the log read.
    pub events: Vec<(f64, String)>,
}

/// The variables each panel starts from.
const PANELS: [&str; 4] = ["HudHealth", "HudSuit", "HudAmmo", "HudAmmoSecondary"];

impl Hl2Hud {
    pub fn new(script: Script, look: &Look) -> Hl2Hud {
        let mut hud = Hl2Hud {
            anim: Animator::new(script, look.screen.units_wide()),
            numeric: Numeric::default(),
            aux: Aux::default(),
            damage: Damage::default(),
            quick: Quick::default(),
            zoom: ZoomView::default(),
            selection: Selection::default(),
            history: History::default(),
            notices: Notices::default(),
            now: 0.0,
            events: Vec::new(),
        };
        hud.init_vars(look);
        hud
    }

    /// Set every variable to where its element starts.
    fn init_vars(&mut self, look: &Look) {
        let (fg, bg) = (look.colour("FgColor"), look.colour("BgColor"));
        self.anim.screen_units = look.screen.units_wide();
        for p in PANELS {
            self.anim.set(p, "FgColor", Value::colour(fg));
            self.anim.set(p, "BgColor", Value::colour(bg));
            self.anim.set(p, "Blur", Value::Number(0.0));
            self.anim.set(p, "Alpha", Value::Number(if matches!(p, "HudSuit" | "HudAmmoSecondary") { 0.0 } else { 255.0 }));
        }
        // The aux panel starts red and opaque (the first start's red flash).
        self.anim.set("HudSuitPower", "AuxPowerColor", Value::colour([255, 0, 0, 255]));
        self.anim.set("HudSuitPower", "BgColor", Value::colour(bg));
        self.anim.set("HudSuitPower", "Alpha", Value::Number(255.0));
        let clear = Value::colour([0, 0, 0, 0]);
        for (p, v) in [
            ("HudWeaponSelection", "FgColor"),
            ("HudWeaponSelection", "TextColor"),
            ("HudDamageIndicator", "DmgColorLeft"),
            ("HudDamageIndicator", "DmgColorRight"),
            ("HudDamageIndicator", "DmgHighColorLeft"),
            ("HudDamageIndicator", "DmgHighColorRight"),
            ("HudDamageIndicator", "DmgFullscreenColor"),
        ] {
            self.anim.set(p, v, clear);
        }
        for v in ["Alpha", "SelectionAlpha"] {
            self.anim.set("HudWeaponSelection", v, Value::Number(0.0));
        }
        self.anim.set("HudWeaponSelection", "TextScan", Value::Number(1.0));
        self.anim.set("TeamDisplay", "Alpha", Value::Number(255.0));
    }

    /// Start a named HUD event now.
    pub fn fire(&mut self, event: &str) {
        self.anim.start(event, self.now);
        self.events.push((self.now, event.to_string()));
        if self.events.len() > 64 {
            self.events.remove(0);
        }
    }

    /// Start a named HUD event as of time `at` (an earlier moment than
    /// this frame's: a timer that fell between two frames).
    pub fn fire_at(&mut self, event: &str, at: f64) {
        self.anim.start(event, at);
        self.events.push((at, event.to_string()));
        if self.events.len() > 64 {
            self.events.remove(0);
        }
    }

    /// The events started at or after `since`, by name.
    pub fn fired(&self, since: f64) -> Vec<&str> {
        self.events.iter().filter(|(t, _)| *t >= since).map(|(_, e)| e.as_str()).collect()
    }

    /// The HUD reset a spawn sends (1 of the spec): animations cancelled,
    /// the elements back to their starts, the history emptied; the suit's
    /// memory and the death notices stay.
    pub fn reset(&mut self, look: &Look) {
        self.anim.cancel_all();
        self.init_vars(look);
        self.numeric.health = None;
        self.numeric.ammo_weapon = None;
        self.numeric.ammo_value = -1;
        self.numeric.ammo_reserve = -1;
        self.numeric.sec_weapon = None;
        self.numeric.sec_value = -1;
        self.aux.reset();
        self.damage = Damage::default();
        self.history.clear();
        self.quick.reset();
        self.selection.close_now();
        self.events.clear();
    }

    /// One frame: the elements start their events on what changed, then
    /// the animations step.
    pub fn think(&mut self, inp: &Inputs, now: f64) {
        self.now = now;
        self.think_numeric(inp);
        self.think_aux(inp);
        self.think_quick(inp);
        self.think_zoom(inp);
        self.think_selection(inp);
        self.history.expire(now);
        self.notices.list.retain(|n| n.until > now);
        self.anim.update(now);
    }

    /// Everything this frame paints, back to front.
    pub fn frame(&self, look: &Look, inp: &Inputs) -> Frame {
        let mut frame = Frame::default();
        if inp.hide_health && inp.hide_misc {
            return frame;
        }
        self.draw_damage(look, inp, &mut frame);
        self.draw_numeric(look, inp, &mut frame);
        self.draw_aux(look, inp, &mut frame);
        self.draw_selection(look, inp, &mut frame);
        if !inp.hide_misc {
            self.draw_history(look, &mut frame);
            self.draw_notices(look, &mut frame);
        }
        self.draw_crosshair(look, inp, &mut frame);
        self.draw_quick(look, inp, &mut frame);
        self.draw_zoom(look, inp, &mut frame);
        self.draw_team_line(look, inp, &mut frame);
        frame
    }
}
