//! The round's win panel (CS:S's `resource/ui/win_round.res`, placed by
//! the HUD layout's `WinPanel_Round`): the winner, why, how long the
//! round was live, the MVP and a fun fact (`rules::rounds::RoundSummary`),
//! in the game's own strings (`winpanel_*`, `funfact_*`). Shown from the
//! round's end until the next round starts (`game_hud::round_banner`);
//! without the install's layout the old centre print shows instead.
//! Left out: the avatar and its glow, the background star (nested in a
//! panel), rounded corners. How long CS:S shows it, and whether a centre
//! print shows with it, is unmeasured (docs/plans/active/parity-gaps.md,
//! capture C7).
//!
//! The round's win panel (`GameMenus::win_panel`, `resource/ui/win_round.res`
//! at the HUD layout's `WinPanel_Round`): winner, reason, live time, MVP and
//! fun fact from `RoundEnded::summary` in the game's `winpanel_*`/`funfact_*`
//! strings (`words`, `fill`).
//!
//! Known shortcuts (docs/tech-debt.md): Round win panel (`client::win_panel`,
//! `rules::rounds::RoundSummary`; layout and strings from the install, rules
//! ours): the MVP follows CS:S's published award reasons (the planter of a bomb
//! that went off, the defuser, the last hostage's rescuer, else the winners'
//! top killer, ties to the lowest entity; none on a draw or with no winner
//! kills); the fun fact is the first that applies of a win without casualties,
//! a win without kills, 2+ headshot kills, 3+ kills, first blood, a round under
//! 30 s, else `funfact_fallback1/2` (CS:S chooses among many more, unmeasured);
//! the title is the layout's red for terrorists, the scheme's `CT_Blue` for
//! CTs, grey for a draw; no avatar, background star or rounded corners; shown
//! until the next round starts, with no centre print. Why it's OK for now:
//! Captures C7 and C12 in docs/plans/active/parity-gaps.md.

use bevy::prelude::*;

use crate::{
    map::hud::{ActiveHud, GameMenus, HudCoord},
    rules::rounds::{ATTACKERS, DEFENDERS, RoundEndReason, RoundEnded},
};

/// The panel's words: the winner, why, the round's time, the MVP line and
/// the fun fact (None: that line is hidden). `name` names a player.
pub fn words(
    menus: &GameMenus,
    end: &RoundEnded,
    name: &dyn Fn(Entity) -> String,
) -> (String, String, String, Option<String>, Option<String>) {
    let s = |token: &str, fallback: &str| menus.string(token, fallback).to_string();
    let title = match end.winner {
        Some(ATTACKERS) => s("winpanel_t_win", "Terrorists Win"),
        Some(DEFENDERS) => s("winpanel_ct_win", "Counter-Terrorists Win"),
        _ => s("winpanel_draw", "Round Draw"),
    };
    let why = match (end.reason, end.winner) {
        (RoundEndReason::Eliminated, Some(ATTACKERS)) => s("winpanel_end_terrorists__kill", "CTs eliminated"),
        (RoundEndReason::Eliminated, _) => s("winpanel_end_cts_win", "Terrorists eliminated"),
        (RoundEndReason::TargetBombed, _) => s("winpanel_end_target_bombed", "Bomb detonated"),
        (RoundEndReason::BombDefused, _) => s("winpanel_end_bomb_defused", "Bomb defused"),
        (RoundEndReason::TargetSaved, _) => s("winpanel_end_target_saved", "Bombing failed"),
        (RoundEndReason::HostagesRescued, _) => s("winpanel_end_all_hostages_rescued", "All hostages rescued"),
        (RoundEndReason::HostagesNotRescued, _) => s("winpanel_end_hostages_not_rescued", "Rescue failed"),
        // The game has no words for time running out on a map without
        // objectives, or for a draw.
        (RoundEndReason::TimeRanOut | RoundEndReason::Draw, _) => String::new(),
    };
    let summary = &end.summary;
    let time = crate::rules::game_end::clock(summary.seconds as f64);
    let mvp = summary.mvp.map(|(who, why)| {
        let (token, fallback) = why.string();
        fill(&s(token, fallback), Some(&name(who)), &[])
    });
    let fact = summary.fun_fact.as_ref().map(|f| {
        let who = f.player.map(name);
        fill(&s(&f.token, ""), who.as_deref(), &f.values)
    });
    (title, why, time, mvp, fact.filter(|f| !f.is_empty()))
}

/// A game string with its `%s1` (a player), `%s2`, `%s3` filled in.
pub fn fill(text: &str, player: Option<&str>, values: &[String]) -> String {
    let mut out = text.replace("%s1", player.unwrap_or(""));
    for (i, v) in values.iter().enumerate() {
        out = out.replace(&format!("%s{}", i + 2), v);
    }
    out
}

/// The panel under `root` (a full-window node) for a window `size`; false
/// without the install's layout.
pub fn spawn(
    commands: &mut Commands,
    root: Entity,
    hud: &ActiveHud,
    fonts: &crate::client::fonts::UiFonts,
    size: Vec2,
    end: &RoundEnded,
    name: &dyn Fn(Entity) -> String,
) -> bool {
    let Some(menus) = hud.0.menus.as_ref() else { return false };
    let Some(layout) = menus.win_panel.as_ref().and_then(|p| menus.layouts.get(p)) else {
        return false;
    };
    let painter = crate::client::vgui::Painter::new(hud, fonts, size.y);
    // Where the HUD layout puts it, else its values (c-110, 287, 220x138).
    let (x, y, wide, tall) = hud.0.panels.get("WinPanel_Round").map_or(
        (HudCoord::Centre(-110.0), HudCoord::Start(287.0), 220.0, 138.0),
        |p| (p.x, p.y, p.wide, p.tall),
    );
    let s = painter.scale;
    let at = Vec2::new(x.resolve(size.x, s), y.resolve(size.y, s));
    let box_size = Vec2::new(wide, tall) * s;
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(at.x),
                top: px(at.y),
                width: px(box_size.x),
                height: px(box_size.y),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let (title, why, time, mvp, fact) = words(menus, end, name);
    // The title in the winners' colour: the layout's red for the
    // terrorists, the scheme's CT blue, grey for a draw (ours).
    let mut layout = layout.clone();
    for c in &mut layout.controls {
        if c.name.eq_ignore_ascii_case("WinLabel") {
            c.fg = match end.winner {
                Some(ATTACKERS) => c.fg,
                Some(DEFENDERS) => Some(hud.0.colors.get("CT_Blue").copied().unwrap_or([153, 204, 255, 255])),
                _ => Some([204, 204, 204, 255]),
            };
        }
    }
    let mut shown = |c: &crate::map::hud::UiControl| {
        let mut sh = crate::client::vgui::Shown::of(c);
        match c.name.to_lowercase().as_str() {
            "winlabel" => sh.text = Some(title.clone()),
            "windescription" => sh.text = Some(why.clone()),
            "timertext" => sh.text = Some(time.clone()),
            "mvp_text" => match &mvp {
                Some(m) => sh.text = Some(m.clone()),
                None => sh.visible = false,
            },
            "mvp_foreground_star" => sh.visible = mvp.is_some(),
            "funfact" => match &fact {
                Some(f) => sh.text = Some(f.clone()),
                None => sh.visible = false,
            },
            "lowerpanel" => sh.visible = fact.is_some(),
            // No avatars.
            "mvp_avatar" | "mvp_avatarglow" => sh.visible = false,
            _ => {}
        }
        sh
    };
    painter.spawn(commands, frame, box_size, &layout, crate::client::vgui::VguiMenu::Spectator, &mut shown);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rounds::{FunFact, Mvp, RoundSummary};

    #[test]
    fn the_panel_fills_the_games_strings() {
        let mut menus = GameMenus::default();
        for (k, v) in [
            ("winpanel_ct_win", "Counter-Terrorists Win"),
            ("winpanel_end_cts_win", "Terrorists eliminated"),
            ("winpanel_mvp_award_kills", "MVP: %s1 for most kills"),
            ("funfact_first_blood", "%s1 drew first blood %s2 seconds into the round."),
        ] {
            menus.strings.insert(k.into(), v.into());
        }
        let p = Entity::from_raw_u32(7).unwrap();
        let end = RoundEnded {
            winner: Some(DEFENDERS),
            reason: RoundEndReason::Eliminated,
            summary: RoundSummary {
                mvp: Some((p, Mvp::Kills)),
                fun_fact: Some(FunFact {
                    token: "funfact_first_blood".into(),
                    player: Some(p),
                    values: vec!["12".into()],
                }),
                seconds: 83.2,
            },
        };
        let (title, why, time, mvp, fact) = words(&menus, &end, &|_| "Bob".into());
        assert_eq!(title, "Counter-Terrorists Win");
        assert_eq!(why, "Terrorists eliminated");
        assert_eq!(time, "1:24");
        assert_eq!(mvp.as_deref(), Some("MVP: Bob for most kills"));
        assert_eq!(fact.as_deref(), Some("Bob drew first blood 12 seconds into the round."));
        // A draw: no MVP, no words for why.
        let draw = RoundEnded {
            winner: None,
            reason: RoundEndReason::Draw,
            summary: RoundSummary::default(),
        };
        let (title, why, _, mvp, fact) = words(&menus, &draw, &|_| "Bob".into());
        assert_eq!((title.as_str(), why.as_str(), mvp, fact), ("Round Draw", "", None, None));
        assert_eq!(fill("%s2 shots were fired that round.", None, &["40".into()]), "40 shots were fired that round.");
    }
}
