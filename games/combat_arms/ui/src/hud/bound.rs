//! The rest of the HUD from the game's own layouts (specs/combat_arms/
//! ui_system.md L1, H, D; hud_crosshair.md D5-D7, D9), bound by control
//! name as `install` binds the health and weapon panels: the places,
//! sizes, fonts, colours and textures are the layouts', the values and
//! what shows are code's.
//!
//! - `FeedPanel` (the main HUD layout's `KillDeath`, D5): one clone of
//!   its line template (`KillLogLine1`) per kill line, 20 ref px apart;
//!   in it `Name1` the killer, `weapon` the weapon icon, `icon` the first
//!   badge (the second slot, at the last badge's x, in a copy of it),
//!   `Name2` the victim, each at the running x of `model::feed_line` (the
//!   layout's positions are samples); the texts' colours the spec's.
//! - `ScorePanel` (the mode indicator layout's `indicator`, D6, D7): the
//!   mode's child (`child_for`, ui_system.md Corrections 6) with
//!   `mode_text`, `goal_text`, `top_round` and `bottom_round` (the
//!   counters, in their highlight colours), `alpha_gauge` and
//!   `bravo_gauge` (the bars; a round mode's with G - 1 divider marks),
//!   `bg_up` or `bg_down` (your row's highlight), `top_title` and
//!   `bottom_title` (the labels where the layout's are samples; a round
//!   mode's living counts), `round_timer` in `RoundTime` and `bomb_timer`
//!   in `BombTime` (D7).
//! - `ActionPanel` (the main HUD layout's `Action`, D9): `ActionDesc`'s
//!   `Message` the action message, re-centred on `ActionDesc` by its
//!   reference width; `ActionGauge` the plant, defuse or custom gauge
//!   (`Gauge` in whole percent, `ActionTitle`, `ActionIcon`: the HUD
//!   layout attributes' icons).
//!
//! Known shortcuts (the most literal reading): controls nothing drives
//! (flag states, intel, the gauges' counts, the feed's `SubInfo`) are
//! hidden, a text whose layout value is a sample (not a string id) and
//! that nothing sets is hidden; the custom gauge has no icon; the
//! divider marks are our stand-in texture (`DIVIDER`: the spec gives
//! their places, not their look).

use bevy::math::{IVec2, Vec2};

use super::{
    FeedLineView,
    model::{Badge, FeedItem, KillIcon, Rgba},
    score::ScorePanelView,
};
use crate::{
    client::canvas::{Scale, WidgetId},
    layout::{FontList, Kind, Layout},
};

/// A layout of the named control alone (cloned under a fresh root) and
/// its id there.
pub fn graft_named(src: &Layout, name: &str) -> Option<(Layout, WidgetId)> {
    let from = src.find(name)?;
    let mut l = Layout::new();
    let id = l.tree.graft(0, &src.tree, from);
    Some((l, id))
}

/// Set a text's string and colour (None: keep).
pub fn set_text(l: &mut Layout, id: Option<WidgetId>, s: Option<&str>, colour: Option<Rgba>) {
    let Some(id) = id else { return };
    if let Kind::Text(t) = &mut l.widget_mut(id).kind {
        if let Some(s) = s {
            t.set = Some(s.to_string());
        }
        if let Some(c) = colour {
            t.colour = c;
        }
    }
}

/// Set an image's texture.
pub fn set_texture(l: &mut Layout, id: Option<WidgetId>, texture: &str) {
    let Some(id) = id else { return };
    if let Kind::Image(look) | Kind::Window(look) = &mut l.widget_mut(id).kind {
        look.texture = texture.to_string();
    }
}

pub fn show(l: &mut Layout, id: Option<WidgetId>, show: bool) {
    if let Some(id) = id {
        l.tree.get_mut(id).show = show;
    }
}

/// Fill a gauge so it draws a bar `bar` ref px wide (W5's floor gives
/// the same width back).
pub fn set_gauge(l: &mut Layout, id: Option<WidgetId>, bar: f32) {
    let Some(id) = id else { return };
    if let Kind::Gauge(g) = &mut l.widget_mut(id).kind {
        let range = g.max_range.max(1) as f64;
        g.fill = if bar <= 0.0 { 0.0 } else { bar as f64 * 100.0 / range + 1e-6 };
    }
}

/// Fill a gauge to `p` whole percent (W5: the bar ⌊p · MaxRange / 100⌋).
pub fn set_percent(l: &mut Layout, id: Option<WidgetId>, p: u32) {
    let Some(id) = id else { return };
    if let Kind::Gauge(g) = &mut l.widget_mut(id).kind {
        g.fill = p as f64;
    }
}

/// Fill a gauge to `fraction` (0 to 1) of its range.
pub fn set_fraction(l: &mut Layout, id: Option<WidgetId>, fraction: f32) {
    let Some(id) = id else { return };
    if let Kind::Gauge(g) = &mut l.widget_mut(id).kind {
        g.fill = (fraction.clamp(0.0, 1.0) * 100.0) as f64;
    }
}

/// Hide every text under `from` whose layout value is a sample (not a
/// `$` string id) and that code hasn't set.
pub fn hide_samples(l: &mut Layout, from: WidgetId) {
    for id in l.tree.order_from(from) {
        let sample = matches!(&l.widget(id).kind, Kind::Text(t) if t.set.is_none() && !t.element.starts_with('$'));
        if sample {
            l.tree.get_mut(id).show = false;
        }
    }
}

// ---------------------------------------------------------------------------
// The kill feed (D5)

/// The kill feed's panel and its line template.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedPanel {
    source: Layout,
    panel: WidgetId,
    line: WidgetId,
}

/// A kill line element's texture: by its icon and badge.
pub struct FeedArt<'a> {
    pub icon: &'a dyn Fn(&KillIcon) -> Option<String>,
    pub badge: &'a dyn Fn(Badge) -> Option<String>,
}

impl FeedPanel {
    /// `KillDeath` of the main HUD layout with its first line; None when
    /// the layout lacks them.
    pub fn new(hud: &Layout) -> Option<Self> {
        let (source, panel) = graft_named(hud, "KillDeath")?;
        let line = source.tree.find_from(panel, "KillLogLine1")?;
        Some(Self { source, panel, line })
    }

    /// The panel with `lines` (their tops in ref px, `FeedLineView::y`) at
    /// `scale`.
    pub fn build(&self, lines: &[FeedLineView], art: &FeedArt, scale: Scale) -> Layout {
        let mut l = Layout::new();
        l.scale = scale;
        let panel = l.tree.graft(0, &self.source.tree, self.panel);
        l.tree.get_mut(panel).show = true;
        let template = l.tree.find_from(panel, "KillLogLine1");
        show(&mut l, template, false);
        let panel_y = l.tree.abs_pos(panel).y;
        for line in lines {
            let id = l.tree.graft(panel, &self.source.tree, self.line);
            let n = l.tree.get_mut(id);
            n.show = true;
            n.pos = IVec2::new(n.pos.x, line.y as i32 - panel_y);
            let find = |l: &Layout, name: &str| l.tree.find_from(id, name);
            let (name1, name2, weapon, icon) = (find(&l, "Name1"), find(&l, "Name2"), find(&l, "weapon"), find(&l, "icon"));
            let id_ = find(&l, "SubInfo");
            show(&mut l, id_, false);
            show(&mut l, weapon, false);
            show(&mut l, icon, false);
            let mut texts = [name1, name2].into_iter();
            let mut badges = 0;
            for item in &line.items {
                let (target, x) = match item {
                    FeedItem::Text { x, text, colour } => {
                        let t = texts.next().flatten();
                        set_text(&mut l, t, Some(text), Some(*colour));
                        (t, *x)
                    }
                    FeedItem::Icon { x, icon: k } => {
                        let Some(tex) = (art.icon)(k) else { continue };
                        set_texture(&mut l, weapon, &tex);
                        show(&mut l, weapon, true);
                        (weapon, *x)
                    }
                    FeedItem::Badge { x, badge } => {
                        let Some(tex) = (art.badge)(*badge) else { continue };
                        // The first badge in `icon`, every later one in a
                        // copy of it (the second slot).
                        let slot = match (badges, icon) {
                            (0, _) => icon,
                            (_, Some(i)) => Some(l.tree.graft(id, &l.tree.clone(), i)),
                            _ => None,
                        };
                        badges += 1;
                        set_texture(&mut l, slot, &tex);
                        show(&mut l, slot, true);
                        (slot, *x)
                    }
                };
                if let Some(t) = target {
                    let n = l.tree.get_mut(t);
                    n.pos.x = x as i32;
                }
            }
        }
        l
    }
}

// ---------------------------------------------------------------------------
// The score panel (D6, D7)

/// The indicator layout's child for game mode `key` (ui_system.md
/// Corrections 6; game_modes.md S13).
pub fn child_for(key: &str) -> &'static str {
    match key {
        "TDM" | "CDM" | "SFM" => "Eli",
        "TSV" | "CTSV" => "Eli_pro",
        "TMM" | "CMM" | "SBT" => "s&d",
        "ESL" => "ESL",
        "CTF" | "CCTF" => "CTF",
        "FFA" => "Indicator_OMA",
        "LMS" => "LMS",
        "BNT" => "Indicator_SPY",
        "QRT" => "QRT",
        "HQM" => "HQT",
        "SQM" => "Squad",
        "SQM2" => "HG2",
        "VTDM" => "VIP_Eli",
        "ESC" | "CESC" => "VIP_Esct",
        "VWRM" => "WeaponRace",
        "HG" => "HungerGame",
        "FRT" => "FRT",
        "FRT_IShip" => "FRT_IShip",
        "BOT" => "Eli_single",
        _ => "Eli",
    }
}

/// The stand-in texture of a round mode's bar divider marks (ours:
/// `ui::CaUi`).
pub const DIVIDER: &str = "ours/round_divider.dtx";

/// The mode indicator layout.
#[derive(Clone, Debug, PartialEq)]
pub struct ScorePanel {
    source: Layout,
}

impl ScorePanel {
    pub fn new(indicator: &Layout) -> Option<Self> {
        indicator.find("indicator")?;
        Some(Self { source: indicator.clone() })
    }

    /// Mode `key`'s panel showing `view` and the round and bomb timers
    /// at `scale`; None when the layout has no child for it.
    pub fn build(&self, key: &str, view: &ScorePanelView, (timer, bomb): (Option<&str>, Option<&str>), scale: Scale) -> Option<Layout> {
        let (mut l, root) = graft_named(&self.source, "indicator")?;
        l.scale = scale;
        let want = child_for(key);
        let children = l.tree.get(root).children.clone();
        let child = children
            .iter()
            .copied()
            .find(|&c| l.tree.get(c).name.eq_ignore_ascii_case(want))?;
        for c in children {
            l.tree.get_mut(c).show = c == child;
        }
        let find = |l: &Layout, name: &str| l.tree.find_from(child, name);
        let id_ = find(&l, "mode_text");
        set_text(&mut l, id_, Some(&view.mode_name), None);
        let id_ = find(&l, "goal_text");
        set_text(&mut l, id_, Some(&view.goal), None);
        for (i, (round, gauge, title)) in [
            ("top_round", "alpha_gauge", "top_title"),
            ("bottom_round", "bravo_gauge", "bottom_title"),
        ]
        .into_iter()
        .enumerate()
        {
            let row = &view.rows[i];
            let id_ = find(&l, round);
            set_text(&mut l, id_, Some(&row.score), Some(row.colour));
            let id_ = find(&l, gauge);
            set_gauge(&mut l, id_, row.bar);
            // A round mode's bars: G - 1 divider marks in the bar's frame.
            if let Some(g) = id_
                && view.dividers > 1
            {
                let (at, size) = (l.tree.get(g).pos.as_vec2(), l.tree.get(g).size.as_vec2());
                let parent = l.tree.get(g).parent.unwrap_or(child);
                for (k, (mark, h)) in super::model::round_dividers(at, size, view.dividers).into_iter().enumerate() {
                    let look = crate::layout::ImageLook {
                        texture: DIVIDER.to_string(),
                        ..Default::default()
                    };
                    let name = format!("divider_{i}_{k}");
                    l.add(parent, &name, mark.as_ivec2(), IVec2::new(1, h as i32), Kind::Image(look));
                }
            }
            // Round modes: each team's living players (Corrections 9).
            if view.titles_are_counts {
                let id_ = find(&l, title);
                set_text(&mut l, id_, Some(&view.labels[i]), None);
                continue;
            }
            // The label: the layout's string id, else ours, unless a
            // sibling carries the label (then this one is a count).
            if let Some(t) = find(&l, title) {
                let is_id = l.widget(t).text().is_some_and(|x| x.element.starts_with('$'));
                let parent = l.tree.get(t).parent.unwrap_or(child);
                let sibling = l.tree.get(parent).children.iter().any(|&s| {
                    s != t && l.widget(s).text().is_some_and(|x| x.element.starts_with('$'))
                });
                if is_id {
                    set_text(&mut l, Some(t), None, Some(row.colour));
                } else if !sibling {
                    set_text(&mut l, Some(t), Some(&view.labels[i]), Some(row.colour));
                }
            }
        }
        let (up, down) = (find(&l, "bg_up"), find(&l, "bg_down"));
        let mine = view.highlight.map(|h| if h.y < 22.0 { 0 } else { 1 });
        show(&mut l, up, mine == Some(0));
        show(&mut l, down, mine == Some(1));
        let round_time = find(&l, "RoundTime");
        show(&mut l, round_time, timer.is_some());
        if let Some(t) = timer {
            let id_ = find(&l, "round_timer");
            set_text(&mut l, id_, Some(t), None);
        }
        // A planted bomb's fuse in its own control (D7, Corrections 8).
        let bomb_time = find(&l, "BombTime");
        show(&mut l, bomb_time, bomb.is_some());
        if let Some(t) = bomb {
            let id_ = find(&l, "bomb_timer");
            set_text(&mut l, id_, Some(t), None);
        }
        hide_samples(&mut l, child);
        Some(l)
    }
}

// ---------------------------------------------------------------------------
// The action panel: the pickup prompt and the action gauge

/// The main HUD layout's `Action`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionPanel {
    source: Layout,
    root: WidgetId,
}

impl ActionPanel {
    pub fn new(hud: &Layout) -> Option<Self> {
        let (source, root) = graft_named(hud, "Action")?;
        Some(Self { source, root })
    }

    /// The panel with the action message, re-centred on `ActionDesc` by
    /// its reference width in `fonts` (D9), and the action gauge (title,
    /// fill in whole percent, icon texture); None when it shows neither.
    pub fn build(
        &self,
        prompt: Option<&str>,
        gauge: Option<(&str, u32, Option<&str>)>,
        scale: Scale,
        fonts: &FontList,
    ) -> Option<Layout> {
        if prompt.is_none() && gauge.is_none() {
            return None;
        }
        let mut l = self.source.clone();
        l.scale = scale;
        let root = self.root;
        l.tree.get_mut(root).show = true;
        let find = |l: &Layout, name: &str| l.tree.find_from(root, name);
        let desc = find(&l, "ActionDesc");
        show(&mut l, desc, prompt.is_some());
        let message = find(&l, "Message");
        set_text(&mut l, message, prompt, None);
        if let (Some(m), Some(d), Some(text)) = (message, desc, prompt) {
            let width = l
                .widget(m)
                .text()
                .and_then(|t| fonts.get(&t.font))
                .map_or(0, |f| crate::layout::text::measure(text, f, 0, scale).reference.x);
            // `Message` in `ActionDesc`'s coordinates: its x so its centre
            // is on `ActionDesc`'s.
            let desc_w = l.tree.get(d).size.x;
            let parent = l.tree.get(m).parent;
            let (desc_abs, parent_abs) = (l.tree.abs_pos(d).x, parent.map_or(0, |p| l.tree.abs_pos(p).x));
            let x = super::model::message_x(desc_abs, desc_w, width) - parent_abs;
            l.tree.get_mut(m).pos.x = x;
        }
        let id_ = find(&l, "bullion_icon");
        show(&mut l, id_, false);
        let bar = find(&l, "ActionGauge");
        show(&mut l, bar, gauge.is_some());
        if let Some((title, percent, icon)) = gauge {
            let id_ = find(&l, "ActionTitle");
            set_text(&mut l, id_, Some(title), None);
            let id_ = find(&l, "Gauge");
            set_percent(&mut l, id_, percent);
            let icon_id = find(&l, "ActionIcon");
            match icon {
                Some(t) => set_texture(&mut l, icon_id, t),
                None => show(&mut l, icon_id, false),
            }
        }
        hide_samples(&mut l, root);
        Some(l)
    }
}

/// A control's absolute reference position (tests).
pub fn at(l: &Layout, id: WidgetId) -> Vec2 {
    l.tree.abs_pos(id).as_vec2()
}
