//! The score panel (hud_crosshair.md D6), the round and bomb timers (D7)
//! and the action gauge (D9), driven by the match
//! (`game::rules::CaMatchView`: the server's, or a network client's
//! copy): the mode's name and its goal text (the string table's,
//! game_modes.md S13: `model::goal_text`), each team's counter with its
//! progress bar (the mode's `Panel`: team counters, round wins; in a
//! free-for-all mode the local player's and the leader's), the local
//! player's row highlighted; in Elimination Pro, Search & Destroy and
//! Bombing Run each team's living players in the row titles and the bars'
//! round dividers (Corrections 9); the timer (Corrections 8): the setup's
//! delay counting down (none when it is 0), the round's time while live,
//! and after a plant the bomb's fuse in its own timer (`BombTime`) with
//! the round timer hidden; at a round's result (until the next round's
//! setup, `result_showing`) the panel, its timers and our objectives'
//! lines hide (Corrections 10). The objectives' lines
//! (`game::rules::objectives`: the planted bomb's site, the flags, the
//! pole, the intel, what you carry; Hired Guns' bars banked, in bags and
//! unclaimed, your pocket, bank and mercenaries with their order; VIP
//! Escort's roles, the VIP, the exit, the rounds until the swap and its
//! notice; the clan modes' half) are ours. The action gauge is the viewed player's
//! (whom a first-person spectator watches): its title the string table's,
//! its fill in whole percent on its own game-clock timer, hidden at its
//! total even while held (`model::ActionGauge`, Corrections 12).
//! `update_score` reads the world into `CaScoreHud` (headless, for the
//! tests); `draw_score` draws it.
//!
//! With the install's layouts the panel is the game's mode indicator
//! (`bound::ScorePanel`) and the gauge the game's action panel
//! (`bound::ActionPanel`); without them, ours at D6's places.
//!
//! Known shortcuts: the round modes show their round wins on the same
//! counters (D6 describes TDM's only); the living counts are counted
//! every frame (the game's on each frag and round start); the objectives'
//! lines are ours (text under the timer: the spec gives their lengths,
//! not their looks), the gauge too without the install; the seize gauge
//! is the custom kind with the default title (the pole's trigger has no
//! title of its own in our data), and the team-change gauge isn't driven
//! (no clan modes); Last Man Standing's and Spy Hunt's own panel fields
//! aren't filled; without the install the panel's textures are flat
//! placeholder boxes, the dividers 1 px dark marks.

use bevy::prelude::*;

use super::{PenParams, colour, hud_text, layer, model::*, pen, rect, rebuild};
use crate::{
    client::{
        canvas::{HAlign, VAlign},
        spectate::SpecView,
    },
    core::{Dead, Health, Intent, LocalPlayer, SimClock, Team},
    game::rules::{
        CaMatchView, CaStats, Panel, Phase,
        escort::CaVip,
        hired_guns::CaBank,
        mercs::{CaMerc, CaSquadOrder, MercOrder},
        objectives::{ActionKind, CaAction, CaCarry, CaObjectives, FlagAt},
    },
    CaUi, layout::Effect,
};

/// The panel as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct ScorePanelView {
    /// The game mode's key (`game::rules`' `TDM`, `CTF`, ...).
    pub mode_key: String,
    pub mode_name: String,
    pub goal: String,
    /// Alpha's and Bravo's rows (a free-for-all mode's: yours, the
    /// leader's).
    pub rows: [ScoreRow; 2],
    /// The rows' labels.
    pub labels: [String; 2],
    /// The highlight's top-left (ref px) for the local player's team.
    pub highlight: Option<Vec2>,
    /// The row titles are counts (each team's living players), not
    /// labels (D6, Corrections 9).
    pub titles_are_counts: bool,
    /// The bars' round dividers: the goal G (G - 1 marks); 0 none.
    pub dividers: u32,
}

/// The action gauge as shown (D9): what, its title (the string table's),
/// its fill in whole percent.
#[derive(Clone, Debug, PartialEq)]
pub struct GaugeView {
    pub kind: GaugeKind,
    pub title: String,
    pub percent: u32,
}

/// The score panel and the round timer, as shown now.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaScoreHud {
    pub panel: Option<ScorePanelView>,
    /// The round timer's text (D7); None: no timer.
    pub timer: Option<String>,
    /// The bomb timer's (`BombTime`): a planted bomb's fuse.
    pub bomb_timer: Option<String>,
    /// The objectives' lines.
    pub objectives: Vec<String>,
    /// The action gauge.
    pub gauge: Option<GaugeView>,
}

/// The panel's places (D6, D7), ref px.
pub mod places {
    use bevy::math::Vec2;

    pub const PANEL: (Vec2, Vec2) = (Vec2::new(342.0, 0.0), Vec2::new(340.0, 115.0));
    pub const MODE: (Vec2, Vec2) = (Vec2::new(347.0, 3.0), Vec2::new(220.0, 16.0));
    pub const GOAL: (Vec2, Vec2) = (Vec2::new(572.0, 3.0), Vec2::new(105.0, 16.0));
    /// Alpha's and Bravo's label and score boxes.
    pub const LABELS: [(Vec2, Vec2); 2] = [
        (Vec2::new(347.0, 23.0), Vec2::new(90.0, 18.0)),
        (Vec2::new(347.0, 43.0), Vec2::new(90.0, 18.0)),
    ];
    pub const SCORES: [(Vec2, Vec2); 2] = [
        (Vec2::new(612.0, 23.0), Vec2::new(65.0, 18.0)),
        (Vec2::new(612.0, 43.0), Vec2::new(65.0, 18.0)),
    ];
    /// The bars' frames; the bar is inset by (1, 1), 8 tall.
    pub const BARS: [(Vec2, Vec2); 2] = [
        (Vec2::new(448.0, 32.0), Vec2::new(157.0, 16.0)),
        (Vec2::new(448.0, 43.0), Vec2::new(157.0, 16.0)),
    ];
    pub const HIGHLIGHT: Vec2 = Vec2::new(336.0, 40.0);
    pub const TIMER_STRIP: (Vec2, Vec2) = (Vec2::new(344.0, 63.0), Vec2::new(336.0, 17.0));
    /// The timer's text, centred at about (444, 60).
    pub const TIMER: (Vec2, Vec2) = (Vec2::new(344.0, 52.0), Vec2::new(200.0, 17.0));
    /// Ours: the objectives' first line, one line's height.
    pub const OBJECTIVES: (Vec2, Vec2) = (Vec2::new(344.0, 84.0), Vec2::new(336.0, 16.0));
    /// Ours: the action gauge's frame (the bar inset by 1).
    pub const GAUGE: (Vec2, Vec2) = (Vec2::new(412.0, 470.0), Vec2::new(200.0, 12.0));
    pub const GAUGE_LABEL: (Vec2, Vec2) = (Vec2::new(412.0, 452.0), Vec2::new(200.0, 16.0));
}

/// The panel's looks (D6, D7).
pub mod looks {
    pub const MODE: &str = "number 14";
    pub const LABEL: &str = "bold 18";
    pub const SCORE: &str = "number 18";
    pub const TIMER: &str = "number 20";
    pub const HEADER: [u8; 4] = [218, 218, 218, 255];
}

/// The objectives' lines (ours: the spec places only the gauges and the
/// dropped flag's seconds; the planted bomb's fuse is the bomb timer):
/// the planted bomb's site, the bomb on the ground, each flag, the pole,
/// the intel; `mine` what the local player carries.
pub fn objective_lines(o: &CaObjectives, mine: CaCarry, now: f64) -> Vec<String> {
    let team = |t: u8| if t == 1 { "Alpha" } else { "Bravo" };
    let secs = |until: f64| ((until - now).max(0.0)).trunc() as u64;
    let mut lines = Vec::new();
    if let Some(p) = o.planted {
        let site = if p.number == 0 { "A" } else { "B" };
        lines.push(format!("Bomb planted at {site}"));
    }
    if o.loose_bomb.is_some() {
        lines.push("The bomb is on the ground".into());
    }
    for f in &o.flags {
        let at = match f.at {
            FlagAt::Home => "home".to_string(),
            FlagAt::Carried => "taken".to_string(),
            FlagAt::Dropped { returns, .. } => format!("dropped, home in {} s", secs(returns)),
        };
        lines.push(format!("{} flag: {at}", team(f.team)));
    }
    if let Some(p) = o.pole {
        lines.push(match p.team {
            Some(t) => format!("{}'s flag rising: {} s", team(t), (now - p.since).max(0.0).trunc() as u64),
            None => "The flag pole is free".into(),
        });
    }
    if o.stage == 1 && !o.intel.is_empty() {
        lines.push(format!("Intel on the ground: {}", o.intel.len()));
    }
    if o.stage == 2 {
        lines.push("The final spy is out".into());
    }
    if let Some(f) = mine.flag {
        lines.push(format!("You carry the {} flag", team(f)));
    }
    if mine.bomb {
        lines.push("You carry the bomb".into());
    }
    if mine.intel > 0 {
        lines.push(format!("You carry {} intel", mine.intel));
    }
    // Hired Guns (game_modes_2.md M11.10; the chat warning of M11.4 as a
    // line).
    if o.placed > 0 || !o.bullion.is_empty() {
        let (level, bags) = o.bars_on_ground();
        lines.push(format!("Banked {} - {}, in bags {bags}, unclaimed {level}", o.banked[0], o.banked[1]));
    }
    if mine.bars > 0 {
        lines.push(format!("Pocket {:02} bars", mine.bars));
        if mine.bars >= 11 {
            let pct = (mine.bars.min(25) - 10) * 3;
            lines.push(format!("Speed penalty {pct} % with {} bars", mine.bars));
        }
    }
    // VIP Escort (M13).
    if o.escort != 0 {
        lines.push(format!("{} escorts the VIP", team(o.escort)));
    }
    if let Some(x) = o.exit.filter(|x| x.open || x.opens.is_finite()) {
        lines.push(if x.open {
            "The exit is open".into()
        } else {
            format!("The exit opens in {} s", secs(x.opens))
        });
    }
    if let Some(n) = o.swap_notice.filter(|n| *n > now) {
        lines.push(format!("Sides swap in {} s", secs(n) + 1));
    } else if o.rounds_to_swap != 0 && o.rounds_to_swap != 255 {
        lines.push(format!("Rounds until the swap: {}", o.rounds_to_swap));
    }
    if o.half != 0 {
        lines.push(format!("Half {}", o.half));
    }
    lines
}

/// Hired Guns' own lines for the local player: its bank, its mercenaries
/// (health, %) and their order (M11.10).
pub fn squad_lines(bank: Option<i32>, order: Option<MercOrder>, units: &[(String, f32)]) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(b) = bank {
        lines.push(format!("Bank {b}"));
    }
    if !units.is_empty() {
        let order = match order.unwrap_or_default() {
            MercOrder::Default => "default",
            MercOrder::FollowMe => "follow me",
            MercOrder::Hold => "hold position",
            MercOrder::AttackMove(_) => "attack move",
        };
        lines.push(format!("Mercenaries ({order}):"));
        for (name, health) in units {
            lines.push(format!("  {name} {:.0} %", health * 100.0));
        }
    }
    lines
}

/// Whether a round's result shows (ui_system.md Corrections 1): from the
/// round's result until the next round's setup (or for good once the
/// match's result is in); the mode panel hides meanwhile (D6,
/// Corrections 10).
pub fn result_showing(phase: Phase) -> bool {
    matches!(phase, Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver)
}

/// The score panel's memory: the round setup's delay (when its setup
/// began, s) and the action gauge's timer.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ScoreMemory {
    pub setup: Option<(f64, f64)>,
    pub gauge: ActionGauge,
}

/// The match into `CaScoreHud`.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_score(
    view: Res<CaMatchView>,
    objectives: Option<Res<CaObjectives>>,
    local: Query<(Entity, &Team, Option<&CaStats>, Option<&CaCarry>), With<LocalPlayer>>,
    actions: Query<&CaAction>,
    spec: Option<Res<SpecView>>,
    squad: (
        Query<(Entity, Option<&CaBank>, Option<&CaSquadOrder>, Has<CaVip>), With<LocalPlayer>>,
        Query<(&CaMerc, Option<&Name>, Option<&crate::core::Health>)>,
        Query<Option<&Name>, With<CaVip>>,
    ),
    everyone: Query<&CaStats>,
    living: Query<(&Team, &Health, Has<Dead>), With<Intent>>,
    (clock, time, ui): (Option<Res<SimClock>>, Res<Time>, Res<CaUi>),
    mut memory: ResMut<ScoreMemory>,
    mut hud: ResMut<CaScoreHud>,
) {
    let now = clock.map_or_else(|| time.elapsed_secs_f64(), |c| c.now);
    let me = local.single().ok();
    // The setup's delay, from its first frame (D7: hidden when it is 0).
    match view.phase {
        Phase::Setup { until } => {
            if memory.setup.is_none_or(|(u, _)| u != until) {
                memory.setup = Some((until, until - now));
            }
        }
        _ => memory.setup = None,
    }
    // The action gauge (D9, Corrections 12): the viewed player's (whom a
    // first-person spectator watches), on its own timer.
    let viewed = spec.as_deref().and_then(|s| s.in_eye).or(me.map(|m| m.0));
    let action = viewed.and_then(|e| actions.get(e).ok()).copied();
    let kind = action.map(|a| match a.kind {
        ActionKind::Plant => GaugeKind::Plant,
        ActionKind::Defuse => GaugeKind::Defuse,
        ActionKind::Seize => GaugeKind::Custom,
    });
    let percent = memory.gauge.frame(kind, action.map_or(0.0, |a| a.length as f64), now);
    let gauge = kind.zip(percent).map(|(kind, percent)| GaugeView {
        kind,
        title: ui.strings.resolve(kind.title()),
        percent,
    });
    let next = match view.mode() {
        None => CaScoreHud::default(),
        Some(mode) => {
            let round_based = mode.timer_shown;
            let mine_stats = me.and_then(|(_, _, s, _)| s.copied()).unwrap_or_default();
            let best = |f: fn(&CaStats) -> u32| everyone.iter().map(f).max().unwrap_or(0);
            let team_side = me.and_then(|(_, t, ..)| match t.0 {
                1 => Some(0),
                2 => Some(1),
                _ => None,
            });
            let (counters, labels, mine): ([u32; 2], [&str; 2], Option<usize>) = match mode.panel {
                Panel::TeamScore => (view.score.map(|s| s.max(0) as u32), ["Alpha", "Bravo"], team_side),
                Panel::RoundWins => (view.round_wins, ["Alpha", "Bravo"], team_side),
                Panel::Kills => ([mine_stats.kills, best(|s| s.kills)], ["You", "Leader"], Some(0)),
                Panel::PlayerRoundWins => {
                    ([mine_stats.round_wins, best(|s| s.round_wins)], ["You", "Leader"], Some(0))
                }
            };
            let (rows, highlight) = score_panel(counters, view.goal, mine);
            // Round modes: each team's living players in the row titles,
            // the bars' round dividers (D6, Corrections 9).
            let counts = rows_count_living(mode.key);
            let labels = if counts {
                let mut alive = [0u32; 2];
                for (t, h, dead) in &living {
                    if let (Some(i), false) = (t.0.checked_sub(1).filter(|i| *i < 2), dead || h.current <= 0.0) {
                        alive[i as usize] += 1;
                    }
                }
                alive.map(|n| n.to_string())
            } else {
                labels.map(String::from)
            };
            let goal = goal_text(mode.key, view.goal, |id| ui.strings.resolve(id));
            // The timer (D7, Corrections 8): the setup's delay counting
            // down (none when it is 0), the round's time while live, none
            // at the result; a planted bomb's fuse in the bomb timer, the
            // round timer hidden.
            let planted = objectives.as_deref().and_then(|o| o.planted).filter(|_| view.phase == Phase::Live);
            let timer = match view.phase {
                Phase::Setup { until } if memory.setup.is_some_and(|(_, delay)| delay > 0.0) => {
                    timer(round_based, (until - now) * 1000.0)
                }
                Phase::Live if planted.is_none() => {
                    view.round_ends.and_then(|ends| timer(round_based, (ends - now) * 1000.0))
                }
                _ => None,
            };
            let bomb_timer = planted.map(|p| round_timer((p.detonates - now) * 1000.0));
            let carry = me.and_then(|(.., c)| c.copied()).unwrap_or_default();
            let panel = (!result_showing(view.phase)).then(|| ScorePanelView {
                mode_key: mode.key.to_string(),
                mode_name: mode.shown_as.to_string(),
                goal,
                rows,
                labels,
                highlight,
                titles_are_counts: counts,
                dividers: if counts { view.goal } else { 0 },
            });
            let mut lines = objectives.map_or_else(Vec::new, |o| objective_lines(&o, carry, now));
            let (local_squad, mercs, vips) = &squad;
            if let Ok((me, bank, order, vip)) = local_squad.single() {
                let units: Vec<(String, f32)> = mercs
                    .iter()
                    .filter(|(m, ..)| m.leader == me && m.died.is_none())
                    .map(|(m, n, h)| {
                        let name = n.map_or_else(|| format!("unit {}", m.unit), |n| n.as_str().to_string());
                        (name, h.map_or(0.0, |h| h.current.max(0.0) / h.max.max(1e-6)))
                    })
                    .collect();
                lines.extend(squad_lines(bank.map(|b| b.0), order.map(|o| o.0), &units));
                if vip {
                    lines.push("You are the VIP".into());
                } else if let Some(name) = vips.iter().next() {
                    lines.push(format!("VIP: {}", name.map_or("?", |n| n.as_str())));
                }
            }
            CaScoreHud {
                timer: panel.as_ref().and(timer),
                bomb_timer: panel.as_ref().and(bomb_timer),
                objectives: if panel.is_some() { lines } else { Vec::new() },
                panel,
                gauge: gauge.clone(),
            }
        }
    };
    let next = CaScoreHud { gauge, ..next };
    if *hud != next {
        *hud = next;
    }
}

/// The panel's layer.
#[derive(Component)]
pub(super) struct ScoreRoot;

pub(super) fn spawn_root(mut commands: Commands) {
    commands.spawn((ScoreRoot, layer(40), crate::client::hud::HudRoot(crate::game::GAME)));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_score(
    hud: Res<CaScoreHud>,
    screen: Res<super::HudScreen>,
    params: PenParams,
    art: Option<Res<crate::draw::CaArt>>,
    bound: Res<super::BoundHud>,
    root: Single<Entity, With<ScoreRoot>>,
    mut last: Local<Option<(CaScoreHud, Screen, bool)>>,
    mut commands: Commands,
) {
    let screen = screen.0;
    let loaded = art.as_ref().is_some_and(|a| !a.0.is_empty());
    if !rebuild(&mut last, (hud.clone(), screen, loaded), *root, &mut commands) {
        return;
    }
    let pen = pen(&params, screen);
    let centred = (HAlign::Centre, VAlign::Centre);
    // The mode panel hides at a round's result (D6); the gauge doesn't.
    let Some(panel) = &hud.panel else {
        if bound.action.is_none() {
            commands.entity(*root).with_children(|p| draw_gauge(p, pen, screen, &hud, centred));
        }
        return;
    };
    // The game's own mode panel, bound by name (`bound::ScorePanel`); the
    // objectives' lines stay ours, the gauge the action panel's.
    let installed = bound
        .score
        .as_ref()
        .and_then(|s| {
            let timers = (hud.timer.as_deref(), hud.bomb_timer.as_deref());
            s.build(&panel.mode_key, panel, timers, super::scale(screen))
        });
    if let Some(layout) = installed {
        let ui = &params.0;
        let draws = layout.draw(&ui.ctx());
        commands.entity(*root).with_children(|p| {
            crate::draw::spawn_draws(p, &draws, &ui.stand_ins, art.as_deref(), params.1.as_deref(), params.2.as_deref());
            for (k, line) in hud.objectives.iter().enumerate() {
                let (at, size) = places::OBJECTIVES;
                let at = at + Vec2::Y * size.y * k as f32;
                hud_text(p, pen, (at, size, Vec2::ZERO), line, looks::MODE, centred, Effect::Shadow, looks::HEADER);
            }
            if bound.action.is_none() {
                draw_gauge(p, pen, screen, &hud, centred);
            }
        });
        return;
    }
    commands.entity(*root).with_children(|p| {
        // PLACEHOLDER backgrounds for the panel's textures.
        p.spawn(rect(screen.rect(places::PANEL.0, places::PANEL.1), Color::srgba(0.05, 0.06, 0.07, 0.6)));
        if let Some(at) = panel.highlight {
            p.spawn(rect(screen.rect(at, places::HIGHLIGHT), Color::srgba(1.0, 1.0, 1.0, 0.08)));
        }
        for ((at, size), text) in [(places::MODE, &panel.mode_name), (places::GOAL, &panel.goal)] {
            hud_text(p, pen, (at, size, Vec2::ZERO), text, looks::MODE, centred, Effect::Shadow, looks::HEADER);
        }
        for (i, row) in panel.rows.iter().enumerate() {
            let label = panel.labels[i].as_str();
            let (at, size) = places::LABELS[i];
            hud_text(p, pen, (at, size, Vec2::ZERO), label, looks::LABEL, centred, Effect::Shadow, row.colour);
            let (at, size) = places::SCORES[i];
            hud_text(p, pen, (at, size, Vec2::ZERO), &row.score, looks::SCORE, centred, Effect::Shadow, row.colour);
            let (at, size) = places::BARS[i];
            p.spawn(rect(screen.rect(at, size), Color::srgba(0.0, 0.0, 0.0, 0.45)));
            if row.bar > 0.0 {
                let fill = if i == 0 { SCORE_ALPHA } else { SCORE_BRAVO };
                p.spawn(rect(screen.rect(at + Vec2::ONE, Vec2::new(row.bar, 8.0)), colour(fill)));
            }
            // PLACEHOLDER look for the round dividers (D6 gives where).
            for (mark, h) in round_dividers(at, size, panel.dividers) {
                p.spawn(rect(screen.rect(mark, Vec2::new(1.0, h)), Color::srgba(0.08, 0.08, 0.08, 0.85)));
            }
        }
        if let Some(t) = hud.timer.as_ref().or(hud.bomb_timer.as_ref()) {
            let (at, size) = places::TIMER_STRIP;
            p.spawn(rect(screen.rect(at, size), Color::srgba(0.0, 0.0, 0.0, 0.45)));
            let (at, size) = places::TIMER;
            hud_text(p, pen, (at, size, Vec2::ZERO), t, looks::TIMER, centred, Effect::Shadow, [255, 255, 255, 255]);
        }
        for (k, line) in hud.objectives.iter().enumerate() {
            let (at, size) = places::OBJECTIVES;
            let at = at + Vec2::Y * size.y * k as f32;
            hud_text(p, pen, (at, size, Vec2::ZERO), line, looks::MODE, centred, Effect::Shadow, looks::HEADER);
        }
        if bound.action.is_none() {
            draw_gauge(p, pen, screen, &hud, centred);
        }
    });
}

/// Ours: the action gauge (its label and bar) when the game's action
/// panel isn't there.
fn draw_gauge(p: &mut ChildSpawnerCommands, pen: super::Pen, screen: Screen, hud: &CaScoreHud, centred: (HAlign, VAlign)) {
    if let Some(g) = &hud.gauge {
        let (at, size) = places::GAUGE_LABEL;
        hud_text(p, pen, (at, size, Vec2::ZERO), &g.title, looks::MODE, centred, Effect::Shadow, looks::HEADER);
        let (at, size) = places::GAUGE;
        p.spawn(rect(screen.rect(at, size), Color::srgba(0.0, 0.0, 0.0, 0.55)));
        let bar = Vec2::new(gauge_bar(g.percent, size.x as i32 - 2) as f32, size.y - 2.0);
        p.spawn(rect(screen.rect(at + Vec2::ONE, bar), Color::srgba(0.9, 0.8, 0.2, 0.9)));
    }
}
