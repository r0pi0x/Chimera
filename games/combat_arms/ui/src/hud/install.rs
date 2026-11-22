//! The HUD's health and weapon panels from the game's own HUD layout
//! (`ui::install::HUD_FILE`; specs/combat_arms/ui_system.md L1, H, D and
//! hud_crosshair.md D2-D4): the panels are the layout's named top-level
//! controls, cloned (`character`, `weaponRoot`), and the HUD's values
//! (`CaHud`'s `StatusView` and `WeaponPanel`) are bound to the named
//! controls in them, so every place, size, font, colour and texture is
//! the layout's and only the values and states are code's.
//!
//! Bindings:
//! - `character`: `bg_HPAP` (the panel and its plates), `HPAP`'s texts
//!   (`Root2` HP and `Root21` AP labels, coloured as D2's labels; `Root22`
//!   SP and `Root221` set labels), `Hp_text` and `AP_text` (the values,
//!   in D2's value colour), `char_sp_gauge` (the stamina bar, D3: its fill
//!   the bar width over the gauge's range), `characterStatus` (the stance
//!   silhouette: `stand_nor` or `crouch_nor`, tinted red at low health).
//! - `weaponRoot`: its `active` panel: `name` (the weapon's name),
//!   `AmmoWnd`'s `Ammo` (clip) and `AmmoClip` (reserve), hidden for
//!   infinite ammo, and `fire_icon` (D4's icon: `firemode_1` single,
//!   `firemode_2` and `firemode_3` bursts, `firemode_auto`).
//!
//! Known shortcuts (the most literal reading): what nothing drives is
//! hidden: `character`'s respawn tokens, fire-team, mutation, squad and
//! ship panels and kill graffiti; `weaponRoot`'s `normal`, `nonActive03`
//! and skill panels, and the active panel's weapon image (`img`, a dummy
//! texture: no weapon-image lookup) and attachment slots; the set bonus
//! value (`Set_Value`: no set bonuses). Which silhouette and fire-mode
//! file stands for which state is our reading of their names. The kill
//! feed, score panel, pickup prompt and action gauge are `bound`'s; the
//! crosshair and name tags stay code's (`ui::hud`).

use bevy::math::IVec2;

use super::model::{FireIcon, Rgba, WeaponPanel};
use super::StatusView;
use crate::client::canvas::{Scale, WidgetId};
use crate::layout::{Kind, Layout};

/// The silhouettes by stance (standing, crouched).
pub const STAND: &str = "ui_hud_character/icon/stand_nor.dtx";
pub const CROUCH: &str = "ui_hud_character/icon/crouch_nor.dtx";

/// A fire-mode icon's texture.
pub fn fire_texture(icon: FireIcon) -> &'static str {
    match icon {
        FireIcon::Single => "ui_hud_weapon/icon/firemode_1.dtx",
        FireIcon::Burst2 => "ui_hud_weapon/icon/firemode_2.dtx",
        FireIcon::Burst3 => "ui_hud_weapon/icon/firemode_3.dtx",
        FireIcon::Auto => "ui_hud_weapon/icon/firemode_auto.dtx",
    }
}

/// The bound controls (None: not in this layout).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    pub character: Option<WidgetId>,
    pub weapon: Option<WidgetId>,
    pub hp: Option<WidgetId>,
    pub ap: Option<WidgetId>,
    pub hp_label: Option<WidgetId>,
    pub ap_label: Option<WidgetId>,
    pub sp_label: Option<WidgetId>,
    pub set_label: Option<WidgetId>,
    pub stamina: Option<WidgetId>,
    pub silhouette: Option<WidgetId>,
    pub name: Option<WidgetId>,
    pub ammo_window: Option<WidgetId>,
    pub clip: Option<WidgetId>,
    pub reserve: Option<WidgetId>,
    pub fire_icon: Option<WidgetId>,
}

/// The panels, cloned out of the game's HUD layout, and their controls.
#[derive(Clone, Debug, PartialEq)]
pub struct HudPanels {
    pub layout: Layout,
    pub controls: Controls,
}

/// Show only the children of `parent` named in `names`.
fn keep(l: &mut Layout, parent: WidgetId, names: &[&str]) {
    for c in l.tree.get(parent).children.clone() {
        let n = l.tree.get_mut(c);
        n.show = names.iter().any(|k| n.name.eq_ignore_ascii_case(k));
    }
}

fn hide(l: &mut Layout, from: Option<WidgetId>, name: &str) {
    if let Some(id) = from.and_then(|f| l.tree.find_from(f, name)) {
        l.tree.get_mut(id).show = false;
    }
}

impl HudPanels {
    /// The panels of `hud` (the game's main HUD layout); None when it has
    /// neither.
    pub fn new(hud: &Layout) -> Option<Self> {
        let (c, w) = (hud.find("character"), hud.find("weaponRoot"));
        if c.is_none() && w.is_none() {
            return None;
        }
        let mut l = Layout::new();
        let character = c.map(|c| l.tree.graft(0, &hud.tree, c));
        let weapon = w.map(|w| l.tree.graft(0, &hud.tree, w));
        let find = |l: &Layout, from: Option<WidgetId>, name: &str| from.and_then(|f| l.tree.find_from(f, name));
        if let Some(c) = character {
            keep(&mut l, c, &["bg_HPAP", "HPAP", "characterStatus"]);
            l.tree.get_mut(c).show = true;
        }
        hide(&mut l, character, "Set_Value");
        let active = find(&l, weapon, "active");
        if let Some(w) = weapon {
            keep(&mut l, w, &["active"]);
            l.tree.get_mut(w).show = true;
        }
        for name in ["img", "img_shadow", "slot", "fire_dtx"] {
            hide(&mut l, active, name);
        }
        let hpap = find(&l, character, "HPAP");
        let controls = Controls {
            character,
            weapon,
            hp: find(&l, character, "Hp_text"),
            ap: find(&l, character, "AP_text"),
            hp_label: find(&l, hpap, "Root2"),
            ap_label: find(&l, hpap, "Root21"),
            sp_label: find(&l, hpap, "Root22"),
            set_label: find(&l, hpap, "Root221"),
            stamina: find(&l, character, "char_sp_gauge"),
            silhouette: find(&l, character, "characterStatus"),
            name: find(&l, active, "name"),
            ammo_window: find(&l, active, "AmmoWnd"),
            clip: find(&l, active, "Ammo"),
            reserve: find(&l, active, "AmmoClip"),
            fire_icon: find(&l, active, "fire_icon"),
        };
        Some(Self { layout: l, controls })
    }

    fn text(&mut self, id: Option<WidgetId>, s: Option<&str>, colour: Option<Rgba>) {
        let Some(id) = id else { return };
        if let Kind::Text(t) = &mut self.layout.widget_mut(id).kind {
            if let Some(s) = s {
                t.set = Some(s.to_string());
            }
            if let Some(c) = colour {
                t.colour = c;
            }
        }
    }

    fn show(&mut self, id: Option<WidgetId>, show: bool) {
        if let Some(id) = id {
            self.layout.tree.get_mut(id).show = show;
        }
    }

    /// Bind this frame's values (None: the panel hidden) at the HUD's
    /// scale.
    pub fn bind(&mut self, status: Option<&StatusView>, weapon: Option<&WeaponPanel>, scale: Scale) {
        self.layout.scale = scale;
        let c = self.controls.clone();
        self.show(c.character, status.is_some());
        if let Some(s) = status {
            self.text(c.hp, Some(&s.hp), Some(s.colours.values));
            self.text(c.ap, Some(&s.ap), Some(s.colours.values));
            self.text(c.hp_label, None, Some(s.colours.hp_ap_labels));
            self.text(c.ap_label, None, Some(s.colours.hp_ap_labels));
            self.text(c.sp_label, None, Some(s.colours.other_labels));
            self.text(c.set_label, None, Some(s.colours.other_labels));
            if let Some(id) = c.stamina
                && let Kind::Gauge(g) = &mut self.layout.widget_mut(id).kind
            {
                let range = g.max_range.max(1) as f64;
                // The bar's width back to a percentage the gauge floors
                // to the same width (W5).
                g.fill = if s.stamina_bar <= 0.0 { 0.0 } else { s.stamina_bar as f64 * 100.0 / range + 1e-6 };
            }
            if let Some(id) = c.silhouette
                && let Kind::Image(look) = &mut self.layout.widget_mut(id).kind
            {
                look.texture = if s.crouched { CROUCH } else { STAND }.to_string();
                look.colour = s.colours.silhouette;
            }
        }
        self.show(c.weapon, weapon.is_some());
        if let Some(w) = weapon {
            self.text(c.name, Some(&w.name), None);
            self.show(c.ammo_window, w.ammo.is_some());
            if let Some((clip, reserve)) = &w.ammo {
                self.text(c.clip, Some(clip), None);
                self.text(c.reserve, Some(reserve), None);
            }
            self.show(c.fire_icon, w.icon.is_some());
            if let Some(icon) = w.icon
                && let Some(id) = c.fire_icon
                && let Kind::Image(look) = &mut self.layout.widget_mut(id).kind
            {
                look.texture = fire_texture(icon).to_string();
            }
        }
    }

    /// A control's absolute reference position (tests).
    pub fn at(&self, id: WidgetId) -> IVec2 {
        self.layout.tree.abs_pos(id)
    }
}
