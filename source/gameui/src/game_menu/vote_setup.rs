//! The vote setup dialog (Source's `CVoteSetupDialog`, hl2's
//! `resource/ui/votehud.res` `VoteSetupDialog`): the issues the server
//! offers in the left list, the chosen issue's choices (players for a
//! kick, the map cycle for a change of level) in the right one, Call Vote
//! and Close. It opens when the server answers `callvote` alone with its
//! issues (`net::vote::VoteView::setups`, Source's `VoteSetup` message):
//! the console's `callvote` (no menu entry: CS:S's `GameMenu.res` has
//! none). Call Vote runs `callvote <issue> <choice>` and closes the menu.
//! Placed and sized by the install's layout (300 x 300, the lists' and
//! buttons' boxes, `linespacing`), the file's numbers without it.
//!
//! Known shortcuts (docs/tech-debt.md): Vote setup dialog: CS:S's game menu
//! has no entry for it (its `GameMenu.res` lists none) and how CS:S opens it
//! is unmeasured: the server answering `callvote` alone opens it (Source's
//! `VoteSetup` message; a guess for CS:S); its title ("Call Vote"), the issues' names ("Kick Player", "Change
//! Level", "Restart Game": CS:S's localization has none) and the lists' look
//! (plain rows, the picked one in the scheme's selection colours, no
//! `SectionedListPanel` headers or scroll bar: a long list scrolls only to
//! keep the focused row shown; the file's `TFThinLineBorder`
//! and rounded corners left out) are ours. Why it's OK for now: capture C25
//! (docs/plans/active/parity-gaps.md).

use super::*;
use crate::net::vote::IssueInfo;

/// The dialog's state: the server's issues, the one picked and its choice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoteSetup {
    pub issues: Vec<IssueInfo>,
    pub issue: Option<usize>,
    pub choice: Option<usize>,
}

impl VoteSetup {
    /// The issues sent by the server: the same issue kept picked (its
    /// choice too while it is still offered).
    pub fn set_issues(&mut self, issues: Vec<IssueInfo>) {
        let name = self.issue.and_then(|i| self.issues.get(i)).map(|i| i.name.clone());
        let value = self.picked_choice().map(|(v, _)| v.clone());
        self.issue = name.and_then(|n| issues.iter().position(|i| i.name == n));
        self.choice = self
            .issue
            .and_then(|i| issues[i].choices.iter().position(|(v, _)| Some(v) == value.as_ref()));
        self.issues = issues;
    }

    fn picked_choice(&self) -> Option<&(String, String)> {
        self.issues.get(self.issue?)?.choices.get(self.choice?)
    }

    /// The `callvote` line Call Vote runs: an issue picked, and its choice
    /// when it has any.
    pub fn line(&self) -> Option<String> {
        let issue = self.issues.get(self.issue?)?;
        if issue.choices.is_empty() {
            return Some(format!("callvote {}", issue.name));
        }
        let (value, _) = self.picked_choice()?;
        Some(format!("callvote {} {value}", issue.name))
    }
}

/// An issue's name in the dialog's list (ours: CS:S's localization has
/// none).
pub fn issue_label(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "kick" => "Kick Player".into(),
        "changelevel" => "Change Level".into(),
        "restartgame" => "Restart Game".into(),
        _ => name.to_string(),
    }
}

/// The dialog's size and its lists' and buttons' boxes (scheme pixels),
/// from the install's layout, else the file's numbers.
pub(super) struct VoteBoxes {
    pub size: (f32, f32),
    pub issues: (f32, f32, f32, f32),
    pub choices: (f32, f32, f32, f32),
    pub call: (f32, f32, f32, f32),
    pub close: (f32, f32, f32, f32),
    pub line: f32,
}

pub(super) fn vote_boxes(menu: &GameMenu) -> VoteBoxes {
    let ui = menu.ui.0.as_ref();
    let frame = ui
        .and_then(|u| u.options.get(source_engine::hud::gameui::VOTE_SETUP.0))
        .and_then(|l| l.get("VoteSetupDialog"));
    let inner = ui.and_then(|u| u.options.get(source_engine::hud::gameui::VOTE_SETUP.1));
    let at = |name: &str, or: (f32, f32, f32, f32)| {
        inner
            .and_then(|l| l.get(name))
            .map_or(or, |c| (coord(c.x), coord(c.y), c.wide, c.tall))
    };
    let line = inner
        .and_then(|l| l.get("VoteSetupList"))
        .and_then(|c| c.keys.get("linespacing"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(22.0);
    VoteBoxes {
        size: frame.map_or((300.0, 300.0), |c| (c.wide, c.tall)),
        issues: at("VoteSetupList", (10.0, 38.0, 128.0, 200.0)),
        choices: at("VoteParameterList", (140.0, 38.0, 150.0, 200.0)),
        call: at("CallVoteButton", (135.0, 250.0, 75.0, 24.0)),
        close: at("Button1", (215.0, 250.0, 75.0, 24.0)),
        line,
    }
}

impl GameMenu {
    /// The dialog's rows: the issues, the picked issue's choices, Call
    /// Vote (once there is something to call), Close.
    pub(super) fn vote_rows(&self) -> Vec<Row> {
        let v = &self.vote;
        let mut rows: Vec<Row> = v
            .issues
            .iter()
            .enumerate()
            .map(|(i, issue)| Self::button_row(&issue_label(&issue.name), Action::VoteIssue(i)))
            .collect();
        if v.issues.is_empty() {
            rows.push(Row::Info("Waiting for the server's issues...".into()));
        }
        if let Some(issue) = v.issue.and_then(|i| v.issues.get(i)) {
            rows.extend(
                issue
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(k, (_, shown))| Self::button_row(shown, Action::VoteChoice(k))),
            );
        }
        rows.push(Row::Button {
            label: "Call Vote".into(),
            action: Action::CallVote,
            enabled: v.line().is_some(),
        });
        // Close: back to the game (or the main menu out of one).
        rows.push(Self::button_row(&self.text("#GameUI_Close", "Close"), Action::Main(MainItem::Resume)));
        rows
    }

    /// The dialog's buttons pressed (`press_action_into`).
    pub(super) fn vote_action(&mut self, action: &Action, out: &mut Outcome) {
        match action {
            Action::VoteIssue(i) => {
                if self.vote.issue != Some(*i) {
                    self.vote.issue = Some(*i);
                    self.vote.choice = None;
                }
            }
            Action::VoteChoice(k) => self.vote.choice = Some(*k),
            Action::CallVote => {
                if let Some(line) = self.vote.line() {
                    out.lines.push(line);
                    self.close(out);
                }
            }
            _ => {}
        }
    }

    /// Open the dialog with the server's issues (an answer to `callvote`
    /// alone): over the menu when it is open on the dialog already.
    pub fn show_vote_setup(&mut self, issues: Vec<IssueInfo>) {
        self.vote.set_issues(issues);
    }
}

/// The dialog's lists and buttons in its frame.
pub(super) fn vote_dialog(commands: &mut Commands, ctx: &Ctx, frame: Entity) {
    let (look, menu) = (ctx.look, ctx.menu);
    let b = vote_boxes(menu);
    let font = look.default_font();
    let lists = [b.issues, b.choices].map(|(x, y, w, h)| {
        commands
            .spawn((
                Node {
                    border: UiRect::all(px(1.0)),
                    overflow: Overflow::clip(),
                    ..place(look, x, y, w, h)
                },
                bevel(look, false),
                BackgroundColor(look.sunken_bg()),
                ChildOf(frame),
            ))
            .id()
    });
    // A list longer than its box shows the focused (else the picked) row:
    // the rows before it scroll off the top.
    let fits = [b.issues.3, b.choices.3].map(|h| ((h - 2.0) / b.line).floor().max(1.0) as usize);
    let keep = [
        list_index(ctx.rows, menu.focus, 0).or(menu.vote.issue),
        list_index(ctx.rows, menu.focus, 1).or(menu.vote.choice),
    ];
    let first = [0, 1].map(|l| keep[l].map_or(0, |k| (k + 1).saturating_sub(fits[l])));
    let mut next = [0usize; 2];
    for (i, row) in ctx.rows.iter().enumerate() {
        let (list, selected, text, color) = match row {
            Row::Button { label: t, action: Action::VoteIssue(k), .. } => {
                (0, menu.vote.issue == Some(*k), t, look.text())
            }
            Row::Button { label: t, action: Action::VoteChoice(k), .. } => {
                (1, menu.vote.choice == Some(*k), t, look.text())
            }
            Row::Info(t) => (0, false, t, look.dull()),
            Row::Button { label: t, action, enabled } => {
                let rect = if *action == Action::CallVote { b.call } else { b.close };
                let state = Btn::enabled(*enabled).focused(menu.focus == i);
                widgets::button(commands, frame, look, rect, t, state, 0, Hit(Target::Row(i), 0));
                continue;
            }
            _ => continue,
        };
        let w = [b.issues.2, b.choices.2][list] - 2.0;
        let k = next[list];
        next[list] += 1;
        if k < first[list] || k >= first[list] + fits[list] {
            continue;
        }
        let y = (k - first[list]) as f32 * b.line;
        let focused = menu.focus == i;
        let bg = if selected { look.selected_bg() } else { Color::NONE };
        let e = commands
            .spawn((
                place(look, 0.0, y, w, b.line),
                BackgroundColor(bg),
                Hit(Target::Row(i), 0),
                widgets::clickable(),
                ChildOf(lists[list]),
            ))
            .id();
        if focused {
            commands.entity(e).insert(Outline::new(px(1.0), px(0.0), look.dark()));
        }
        let color = if selected { look.selected_text() } else { color };
        label(commands, e, look, (4.0, 0.0, w - 8.0, b.line), text, font.clone(), color, -1);
    }
}

/// Which entry of list `list` (0 the issues, 1 the choices) row `i` is.
fn list_index(rows: &[Row], i: usize, list: usize) -> Option<usize> {
    match rows.get(i)? {
        Row::Button { action: Action::VoteIssue(k), .. } if list == 0 => Some(*k),
        Row::Button { action: Action::VoteChoice(k), .. } if list == 1 => Some(*k),
        _ => None,
    }
}
