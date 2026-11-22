//! The vote panels (Source's `resource/ui/votehud.res`, hl2's: CS:S has
//! no copy of its own): the running vote (`VoteActive`: "%s1 wants to call
//! a vote:", the issue, "Press F1 to vote YES" / "Press F2 to vote NO" with
//! our choice highlighted, the yes and no counts), then for a few seconds
//! how it ended (`VotePassed` with what happens now, `VoteFailed` with why)
//! or why our call was refused (`CallVoteFailed`), in the GameUI's words
//! (`GameUI_vote_*`). Drawn from `net::vote::VoteView`, at the file's
//! place (the left edge, 80 px above the middle at 480 high).
//!
//! Known shortcuts (docs/tech-debt.md): Vote panels (`ui::vote_hud`): CS:S's
//! own look is unmeasured: its HUD layout turns the vote element off
//! (`CHudVote` visible 0 in `scripts/hudlayout.res`) and the only panel file
//! is hl2's, which says it is "likely out-of-date", so the panels are that
//! file's as it stands (its second divider crosses the options), without its
//! border (`TFThinLineBorder`, TF2's), the vote bar's boxes (counts as
//! numbers in its count labels instead) and rounded corners as a 6 px radius;
//! the issues' words ("Kick player: %s1?", "Kicking player: %s1...") are ours
//! (CS:S's localization has none); the outcome and a refusal show 5 s (ours).
//! Why it's OK for now: a capture of CS:S's vote panel
//! (docs/plans/active/windows-captures.md).

use bevy::prelude::*;

use crate::{
    map::hud::{ActiveHud, GameMenus, UiControl},
    net::vote::{CallRefusal, VoteFailure, VoteNews, VoteState, VoteView},
};

/// Seconds a vote's outcome or a refusal stays up (ours).
pub const RESULT_TIME: f64 = 5.0;

pub struct VoteHudPlugin;

impl Plugin for VoteHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw);
    }
}

#[derive(Component)]
struct VotePanel;

/// What the panels show now.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    Active(VoteState),
    Passed(String),
    Failed(String),
    Refused(String),
}

/// A game string, else `fallback`.
type Words<'a> = &'a dyn Fn(&str, &str) -> String;

/// The issue as the running vote's panel asks it (ours: CS:S has no
/// words for issues).
pub fn issue_text(issue: &str, detail: &str) -> String {
    match issue.to_lowercase().as_str() {
        "kick" => format!("Kick player: {detail}?"),
        "changelevel" => format!("Change the level to {detail}?"),
        "restartgame" => "Restart the game?".into(),
        _ if detail.is_empty() => format!("{issue}?"),
        _ => format!("{issue} {detail}?"),
    }
}

/// What happens now that it passed (ours).
pub fn passed_text(issue: &str, detail: &str) -> String {
    match issue.to_lowercase().as_str() {
        "kick" => format!("Kicking player: {detail}..."),
        "changelevel" => format!("Changing the level to {detail}..."),
        "restartgame" => "Restarting the game...".into(),
        _ => format!("{issue} {detail}"),
    }
}

/// `seconds` in the game's words for a wait: "%s1 seconds", or in
/// (whole, rounded up) minutes from a minute up.
fn wait(words: Words, token: &str, seconds: u32) -> String {
    let (token, n) = if seconds < 60 {
        (token.to_string(), seconds)
    } else {
        let m = seconds.div_ceil(60);
        (format!("{token}_{}", if m == 1 { "min" } else { "mins" }), m)
    };
    let fallback = format!("You cannot call a vote for {n} {}.", if seconds < 60 { "seconds" } else { "minutes" });
    words(&token, &fallback).replace("%s1", &n.to_string())
}

/// Why a call was refused, in the GameUI's words.
pub fn refusal_text(words: Words, r: &CallRefusal) -> String {
    let w = |t: &str, f: &str| words(t, f);
    match r {
        CallRefusal::Disabled => w("GameUI_Vote_System_Disabled", "Vote System Disabled"),
        CallRefusal::IssueDisabled => w("GameUI_vote_failed_disabled_issue", "Server has disabled that issue."),
        CallRefusal::InProgress => w("GameUI_vote_failed_vote_in_progress", "A vote is currently in progress."),
        CallRefusal::Spam(s) => wait(words, "GameUI_vote_failed_vote_spam", *s),
        CallRefusal::Recently(s) => wait(words, "GameUI_vote_failed_recently", *s),
        CallRefusal::Spectator => w(
            "GameUI_vote_failed_spectator",
            "This server has disabled voting for Spectators.",
        ),
        CallRefusal::Loading => w(
            "GameUI_vote_failed_transition_vote_player",
            "You cannot call a new vote while you are still loading.",
        ),
        CallRefusal::CannotKickAdmin => w(
            "GameUI_vote_failed_cannot_kick_admin",
            "You may not vote to kick the server admin.",
        ),
        CallRefusal::KickYourself => w(
            "GameUI_vote_failed_kick_its_you",
            "You cannot currently call a vote to kick this player.",
        ),
        CallRefusal::MapNameRequired => w("GameUI_vote_failed_map_name_required", "You must specify a map name."),
        CallRefusal::MapNotFound => w("GameUI_vote_failed_map_not_found", "That map does not exist."),
        CallRefusal::MapNotValid => w(
            "GameUI_vote_failed_map_not_valid",
            "That map was not found in the server's map cycle.",
        ),
        CallRefusal::InvalidArgument => w("GameUI_vote_failed_invalid_argument", "Invalid argument used for vote."),
        // Ours (the console's `callvote` refuses it before asking).
        CallRefusal::NoGame => "Votes need a network game.".into(),
    }
}

/// Why a vote failed, in the GameUI's words.
pub fn failure_text(words: Words, f: VoteFailure) -> String {
    match f {
        VoteFailure::Quorum => words("GameUI_vote_failed_quorum", "Not enough players voted."),
        VoteFailure::YesNo => words("GameUI_vote_failed_yesno", "Not enough players voted \"Yes.\""),
    }
}

/// What to show at `now` (real seconds): a refusal just told, else the
/// running vote, else how the last one ended, for `RESULT_TIME`.
pub fn shown(view: &VoteView, now: f64, words: Words) -> Option<Shown> {
    let recent = |at: f64| now - at < RESULT_TIME;
    if let Some((r, at)) = &view.refused
        && recent(*at)
    {
        return Some(Shown::Refused(refusal_text(words, r)));
    }
    if let Some(v) = &view.active {
        return Some(Shown::Active(v.clone()));
    }
    match &view.ended {
        Some((VoteNews::Ended { issue, detail, failure, .. }, at)) if recent(*at) => Some(match failure {
            None => Shown::Passed(passed_text(issue, detail)),
            Some(f) => Shown::Failed(failure_text(words, *f)),
        }),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw(
    time: Res<Time<Real>>,
    view: Option<Res<VoteView>>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<crate::client::fonts::UiFonts>,
    windows: Query<&Window>,
    panels: Query<Entity, With<VotePanel>>,
    mut built: Local<Option<(Option<Shown>, Vec2)>>,
    mut commands: Commands,
) {
    let size = windows.iter().next().map_or(Vec2::new(640.0, 480.0), |w| Vec2::new(w.width(), w.height()));
    let menus = hud.as_ref().and_then(|h| h.0.menus.as_ref());
    let words = |t: &str, f: &str| menus.map_or(f, |m| m.string(t, f)).to_string();
    let now_shown = view.as_ref().and_then(|v| shown(v, time.elapsed_secs_f64(), &words));
    let key = (now_shown.clone(), size);
    if built.as_ref() == Some(&key) {
        return;
    }
    *built = Some(key);
    for e in &panels {
        commands.entity(e).despawn();
    }
    let (Some(what), Some(hud)) = (now_shown, hud) else { return };
    let root = commands
        .spawn((
            VotePanel,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            GlobalZIndex(45),
        ))
        .id();
    if !panel(&mut commands, root, &hud, &fonts, size, &what) {
        // Without the install's panels: the words alone.
        let text = match &what {
            Shown::Active(v) => format!(
                "{} wants to call a vote:\n{}\nF1: Yes ({})  F2: No ({})",
                v.caller,
                issue_text(&v.issue, &v.detail),
                v.yes,
                v.no
            ),
            Shown::Passed(t) => format!("Vote Passed\n{t}"),
            Shown::Failed(t) | Shown::Refused(t) => format!("Vote Failed\n{t}"),
        };
        commands.spawn((
            Text::new(text),
            fonts.client("Default", size.y, 12.0),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.47)),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: Val::Percent(35.0),
                padding: UiRect::all(px(6.0)),
                ..default()
            },
            ChildOf(root),
        ));
    }
}

/// The install's panel for `what` under `root`; false without it.
fn panel(
    commands: &mut Commands,
    root: Entity,
    hud: &ActiveHud,
    fonts: &crate::client::fonts::UiFonts,
    size: Vec2,
    what: &Shown,
) -> bool {
    let Some(menus) = hud.0.menus.as_ref() else { return false };
    let Some(file) = menus.vote_hud.as_ref() else { return false };
    let name = match what {
        Shown::Active(_) => "VoteActive",
        Shown::Passed(_) => "VotePassed",
        Shown::Failed(_) => "VoteFailed",
        Shown::Refused(_) => "CallVoteFailed",
    };
    let (Some(outer), Some(inner)) = (menus.layouts.get(file), menus.layouts.get(&format!("{file}#{name}"))) else {
        return false;
    };
    let painter = crate::client::vgui::Painter::new(hud, fonts, size.y);
    let rects = outer.rects(Rect::from_corners(Vec2::ZERO, size), painter.scale);
    let Some((frame, r)) = outer
        .controls
        .iter()
        .zip(&rects)
        .find(|(c, _)| c.name.eq_ignore_ascii_case(name))
    else {
        return false;
    };
    let bg = frame.bg.map_or(Color::srgba_u8(0, 0, 0, 120), |[r, g, b, a]| Color::srgba_u8(r, g, b, a));
    let e = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                border_radius: BorderRadius::all(px(6.0 * painter.scale)),
                ..default()
            },
            BackgroundColor(bg),
            ChildOf(root),
        ))
        .id();
    let mut fill = |c: &UiControl| fill_control(menus, c, what);
    painter.spawn(commands, e, r.size(), inner, crate::client::vgui::VguiMenu::Spectator, &mut fill);
    true
}

/// A panel's control as `what` shows it.
pub fn fill_control(menus: &GameMenus, c: &UiControl, what: &Shown) -> crate::client::vgui::Shown {
    let mut s = crate::client::vgui::Shown::of(c);
    let words = |t: &str, f: &str| menus.string(t, f).to_string();
    let n = c.name.to_lowercase();
    match what {
        Shown::Active(v) => match n.as_str() {
            "header" => s.text = Some(c.text.replace("%s1", &v.caller)),
            "issue" => s.text = Some(issue_text(&v.issue, &v.detail)),
            "labeloption1" => {
                s.text = Some(words("GameUI_vote_yes_pc_instruction", "Press F1 to vote YES"));
                s.visible = v.may_vote;
            }
            "labeloption2" => {
                s.text = Some(words("GameUI_vote_no_pc_instruction", "Press F2 to vote NO"));
                s.visible = v.may_vote;
            }
            "option1background_selected" => s.visible = v.mine == Some(1),
            "option2background_selected" => s.visible = v.mine == Some(2),
            "option1countlabel" => s.text = Some(v.yes.to_string()),
            "option2countlabel" => s.text = Some(v.no.to_string()),
            // Two options only; the vote bar's boxes aren't drawn.
            n if n.starts_with("labeloption") || n.starts_with("option") || n == "votebar" => s.visible = false,
            _ => {}
        },
        Shown::Passed(t) => {
            if n == "passedresult" {
                s.text = Some(t.clone());
            }
        }
        Shown::Failed(t) | Shown::Refused(t) => {
            if n == "failedreason" {
                s.text = Some(t.clone());
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(_: &str, f: &str) -> String {
        f.to_string()
    }

    #[test]
    fn refusals_read_as_the_gameuis_words() {
        assert_eq!(
            refusal_text(&plain, &CallRefusal::Spam(42)),
            "You cannot call a vote for 42 seconds."
        );
        let game = |t: &str, f: &str| match t {
            "GameUI_vote_failed_recently_mins" => {
                "This vote was called recently and cannot be called again for %s1 minutes.".into()
            }
            "GameUI_vote_failed_recently_min" => {
                "This vote was called recently and cannot be called again for %s1 minute.".into()
            }
            _ => f.to_string(),
        };
        assert_eq!(
            refusal_text(&game, &CallRefusal::Recently(300)),
            "This vote was called recently and cannot be called again for 5 minutes."
        );
        assert_eq!(
            refusal_text(&game, &CallRefusal::Recently(60)),
            "This vote was called recently and cannot be called again for 1 minute."
        );
        assert_eq!(
            refusal_text(&plain, &CallRefusal::CannotKickAdmin),
            "You may not vote to kick the server admin."
        );
    }

    #[test]
    fn a_refusal_then_the_vote_then_its_end_show_for_a_while() {
        let mut view = VoteView::default();
        assert_eq!(shown(&view, 10.0, &plain), None);
        view.active = Some(VoteState {
            issue: "kick".into(),
            detail: "Bob".into(),
            ..default()
        });
        assert!(matches!(shown(&view, 10.0, &plain), Some(Shown::Active(_))));
        view.refused = Some((CallRefusal::InProgress, 9.0));
        assert_eq!(
            shown(&view, 10.0, &plain),
            Some(Shown::Refused("A vote is currently in progress.".into()))
        );
        assert!(matches!(shown(&view, 20.0, &plain), Some(Shown::Active(_))));
        view.active = None;
        view.ended = Some((
            VoteNews::Ended {
                id: 1,
                issue: "kick".into(),
                detail: "Bob".into(),
                failure: None,
            },
            20.0,
        ));
        assert_eq!(shown(&view, 21.0, &plain), Some(Shown::Passed("Kicking player: Bob...".into())));
        assert_eq!(shown(&view, 26.0, &plain), None);
    }
}
