//! The mission display: the timers, caption, message and objective list
//! the map's event objects tell the clients (`game::objects::mission::
//! CaMission`). **Ours, not from the original**: the original client's
//! handlers aren't in the reference (world_objects.md Open question 15),
//! so the places, looks and times are our choices
//! (`data/ui/mission_hud.toml`), in the install's HUD fonts and the HUD's
//! colours, scaled like the rest of the HUD (`Screen`, and the text by
//! `layout::glyph_stretch` through `hud_text`). DESIGN.md "The mission
//! display" says why.
//!
//! - The timers: top centre under the score panel (below its objective
//!   lines), `mm:ss` counting down, in the warning colour for the last
//!   seconds; several stack, with names.
//! - The caption: bottom centre, for its showtime. The message: centre
//!   print, for its showtime. Either is for its team only when the notice
//!   names one.
//! - The objective list: top left, each with a mark for its state and a
//!   count when it has one; the objectives of the local player's team and
//!   everyone's. Always shown while there are objectives.
//!
//! `update_mission` turns the state into `CaMissionHud` (headless, for the
//! tests: `view`); `draw_mission` draws it (`boxes` are the places).
//!
//! Known shortcuts: no key hides the objective list (the options data has
//! none); texts not in the string table show as their ids; captions and
//! messages are wrapped by character count, not measured.

use bevy::prelude::*;
use serde::Deserialize;

use super::{HudScreen, PenParams, hud_text, layer, model::*, pen, rebuild, score};
use crate::{
    client::canvas::{HAlign, VAlign},
    core::{LocalPlayer, SimClock, Team},
    game::objects::{
        events::{Objective, ObjectiveState},
        mission::{CaMission, Shown},
    },
    ui::{
        CaUi,
        layout::{Effect, text::EFFECTS},
    },
};

/// The display's data (`data/ui/mission_hud.toml`).
#[derive(Resource, Clone, Debug, Deserialize)]
pub struct MissionHudData {
    pub timer: TimerData,
    pub caption: ShownData,
    pub message: ShownData,
    pub objectives: ObjectivesData,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TimerData {
    pub at: [f32; 2],
    pub size: [f32; 2],
    pub font: String,
    pub colour: Rgba,
    pub effect: String,
    pub warn_below: f64,
    pub warn_colour: Rgba,
    pub below_score_lines: bool,
    pub step: f32,
    pub show_names: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShownData {
    pub at: [f32; 2],
    pub size: [f32; 2],
    pub font: String,
    pub colour: Rgba,
    pub effect: String,
    pub wrap: usize,
    pub default_time: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ObjectivesData {
    pub at: [f32; 2],
    pub size: [f32; 2],
    pub font: String,
    pub title: String,
    pub title_colour: Rgba,
    pub effect: String,
    pub normal: Rgba,
    pub in_progress: Rgba,
    pub success: Rgba,
    pub failed: Rgba,
    pub mark_normal: String,
    pub mark_in_progress: String,
    pub mark_success: String,
    pub mark_failed: String,
}

impl MissionHudData {
    /// Our data.
    pub fn ours() -> Self {
        Self::parse(include_str!("../../../data/ui/mission_hud.toml")).expect("data/ui/mission_hud.toml")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

impl Default for MissionHudData {
    fn default() -> Self {
        Self::ours()
    }
}

/// A font effect by its name (case ignored); none for an unknown one.
fn effect_of(name: &str) -> Effect {
    EFFECTS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map_or(Effect::None, |(_, e)| *e)
}

/// One timer as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerLine {
    pub text: String,
    /// In the warning colour.
    pub warn: bool,
}

/// One objective as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveLine {
    pub text: String,
    pub state: ObjectiveState,
}

/// The mission display as shown now.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaMissionHud {
    pub timers: Vec<TimerLine>,
    /// The caption's and message's wrapped lines.
    pub caption: Vec<String>,
    pub message: Vec<String>,
    pub objectives: Vec<ObjectiveLine>,
}

/// `mm:ss` of `seconds` left (whole seconds, rounded up; 0 at the end).
pub fn clock_text(seconds: f64) -> String {
    let s = seconds.max(0.0).ceil() as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// `text` broken at spaces into lines of at most `width` characters (a
/// longer word stays whole).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= width => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// An objective's line: its mark, its text, its count.
fn objective_line(o: &Objective, data: &ObjectivesData, resolve: &dyn Fn(&str) -> String) -> ObjectiveLine {
    let mark = match o.state {
        ObjectiveState::Normal => &data.mark_normal,
        ObjectiveState::InProgress => &data.mark_in_progress,
        ObjectiveState::Success => &data.mark_success,
        ObjectiveState::Failed => &data.mark_failed,
    };
    let count = match o.count {
        Some((n, max)) if max > 0 => format!(" ({n}/{max})"),
        Some((n, _)) => format!(" ({n})"),
        None => String::new(),
    };
    ObjectiveLine {
        text: format!("{mark}{}{count}", resolve(&o.id)),
        state: o.state,
    }
}

/// The state at `now` for a player on `team` (None: no team), with
/// `resolve` as the string table.
pub fn view(m: &CaMission, data: &MissionHudData, now: f64, team: Option<u8>, resolve: &dyn Fn(&str) -> String) -> CaMissionHud {
    let for_me = |t: u8| t == 0 || team.is_none_or(|mine| mine == t);
    let mut timers: Vec<&_> = m.timers.iter().collect();
    timers.sort_by(|a, b| a.ends.total_cmp(&b.ends));
    let several = timers.len() > 1;
    let timers = timers
        .into_iter()
        .map(|t| {
            let left = t.ends - now;
            let time = clock_text(left);
            TimerLine {
                text: if several && data.timer.show_names {
                    format!("{} {time}", t.name)
                } else {
                    time
                },
                warn: left <= data.timer.warn_below,
            }
        })
        .collect();
    let shown = |s: &Option<Shown>, d: &ShownData| {
        s.as_ref()
            .filter(|s| s.until > now && for_me(s.team))
            .map_or_else(Vec::new, |s| wrap(&resolve(&s.text), d.wrap))
    };
    CaMissionHud {
        timers,
        caption: shown(&m.caption, &data.caption),
        message: shown(&m.message, &data.message),
        objectives: m
            .objectives
            .iter()
            .filter(|o| for_me(o.team))
            .map(|o| objective_line(o, &data.objectives, resolve))
            .collect(),
    }
}

/// What kind of piece a box is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Timer { warn: bool },
    Caption,
    Message,
    ObjectivesTitle,
    Objective(ObjectiveState),
}

/// One drawn text: its box in reference px, its text, look and colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Boxed {
    pub piece: Piece,
    pub at: Vec2,
    pub size: Vec2,
    pub text: String,
    pub font: String,
    pub colour: Rgba,
    pub effect: Effect,
    pub align: HAlign,
}

/// The display's boxes (reference px). `score_lines`: how many objective
/// lines the score panel shows (the timers go under them).
pub fn boxes(hud: &CaMissionHud, data: &MissionHudData, score_lines: usize) -> Vec<Boxed> {
    let mut out = Vec::new();
    let t = &data.timer;
    let (lines_at, line_h) = score::places::OBJECTIVES;
    let mut y = t.at[1];
    if t.below_score_lines {
        y = y.max(lines_at.y + line_h.y * score_lines as f32 + 2.0);
    }
    for line in &hud.timers {
        out.push(Boxed {
            piece: Piece::Timer { warn: line.warn },
            at: Vec2::new(t.at[0], y),
            size: Vec2::from(t.size),
            text: line.text.clone(),
            font: t.font.clone(),
            colour: if line.warn { t.warn_colour } else { t.colour },
            effect: effect_of(&t.effect),
            align: HAlign::Centre,
        });
        y += t.step;
    }
    for (lines, d, piece) in [(&hud.caption, &data.caption, Piece::Caption), (&hud.message, &data.message, Piece::Message)] {
        for (i, line) in lines.iter().enumerate() {
            out.push(Boxed {
                piece,
                at: Vec2::new(d.at[0], d.at[1] + d.size[1] * i as f32),
                size: Vec2::from(d.size),
                text: line.clone(),
                font: d.font.clone(),
                colour: d.colour,
                effect: effect_of(&d.effect),
                align: HAlign::Centre,
            });
        }
    }
    let o = &data.objectives;
    if !hud.objectives.is_empty() {
        let mut row = |i: usize, piece, text: &str, colour| {
            out.push(Boxed {
                piece,
                at: Vec2::new(o.at[0], o.at[1] + o.size[1] * i as f32),
                size: Vec2::from(o.size),
                text: text.to_string(),
                font: o.font.clone(),
                colour,
                effect: effect_of(&o.effect),
                align: HAlign::Left,
            });
        };
        row(0, Piece::ObjectivesTitle, &o.title, o.title_colour);
        for (i, line) in hud.objectives.iter().enumerate() {
            let colour = match line.state {
                ObjectiveState::Normal => o.normal,
                ObjectiveState::InProgress => o.in_progress,
                ObjectiveState::Success => o.success,
                ObjectiveState::Failed => o.failed,
            };
            row(i + 1, Piece::Objective(line.state), &line.text, colour);
        }
    }
    out
}

/// The state into `CaMissionHud`.
pub(super) fn update_mission(
    mission: Res<CaMission>,
    data: Res<MissionHudData>,
    local: Query<&Team, With<LocalPlayer>>,
    switch: Option<Res<crate::game::options::CaHudSwitch>>,
    (clock, time, ui): (Option<Res<SimClock>>, Res<Time>, Res<CaUi>),
    mut hud: ResMut<CaMissionHud>,
) {
    let now = clock.map_or_else(|| time.elapsed_secs_f64(), |c| c.now);
    let next = if switch.is_some_and(|s| !s.panels()) {
        CaMissionHud::default()
    } else {
        let team = local.iter().next().map(|t| t.0).filter(|t| *t != 0);
        view(&mission, &data, now, team, &|id| ui.strings.resolve(id))
    };
    if *hud != next {
        *hud = next;
    }
}

/// The display's layer.
#[derive(Component)]
pub(super) struct MissionRoot;

pub(super) fn spawn_root(mut commands: Commands) {
    commands.spawn((MissionRoot, layer(39), crate::client::hud::HudRoot(crate::game::GAME)));
}

pub(super) fn draw_mission(
    hud: Res<CaMissionHud>,
    data: Res<MissionHudData>,
    score_hud: Res<score::CaScoreHud>,
    screen: Res<HudScreen>,
    params: PenParams,
    root: Single<Entity, With<MissionRoot>>,
    mut last: Local<Option<(CaMissionHud, usize, Screen)>>,
    mut commands: Commands,
) {
    let screen = screen.0;
    let lines = score_hud.objectives.len();
    if !rebuild(&mut last, (hud.clone(), lines, screen), *root, &mut commands) {
        return;
    }
    let pen = pen(&params, screen);
    commands.entity(*root).with_children(|p| {
        for b in boxes(&hud, &data, lines) {
            hud_text(p, pen, (b.at, b.size, Vec2::ZERO), &b.text, &b.font, (b.align, VAlign::Centre), b.effect, b.colour);
        }
    });
}
