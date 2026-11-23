//! The first run: from an invite to the main menu, signed in (feature
//! `present`; docs/plans/active/onboarding.md, the approved mockups and
//! the user's decisions of 2026-11-21). Drawn in the Signal look over the
//! main menu (`ONBOARDING_Z`) on the main menu's own ground
//! (`chimera_ui::shell::menu_ground`), its words from `data/onboarding.toml`
//! (`Words`), its colours and sizes the theme's (places:
//! `[layout.onboarding]`). The bar carries the game's own lockup (the
//! main menu's: `Ui::lockup_drawn`, `OnboardingLockup`), the screen's
//! place after it; the explaining panes are blueprint plates with corner
//! brackets; every field is the kit's hero field.
//!
//! - **When** (`maybe_start`, the app's start only: never a tool run or a
//!   test): the run starts at the main menu and this PC has no account
//!   and no skip mark (`OnboardingFiles`, `first_run_due`): an account is
//!   the only way onto a hub, so nothing else counts.
//! - **Intro** (decision 7; `start_first_run`, `intro_due`): on the
//!   first run's first start only (no `onboarding-intro-seen` mark beside
//!   `account.toml`, written when it ends or is skipped), a 2.6 s
//!   broadcast-style title over Welcome (`IntroRoot`, `draw_intro`). Its
//!   clock starts once the app is warm (`Intro::warming`,
//!   `chimera_ui::Warmth`: 10 frames in a row under 50 ms, or 6 s of
//!   frames), its first frame standing still until then, so the start's
//!   slow frames never eat its first second; a hitch while it plays moves
//!   it 0.1 s at most (`chimera_ui::motion::MAX_STEP`).
//!   0-0.4 s the blueprint hairlines flicker on from the middle, the bar
//!   (channel and app, ON AIR, a running timecode) and its rule draw out;
//!   0.05-0.9 s the logo pass (accent hairline, the hero slab carrying the
//!   mark, accent and light bands on the slant) sweeps, standing over the
//!   middle while the lockup comes up under it; 0.4-1.0 s the game's own
//!   lockup (`Ui::lockup_parts`, the same nodes as `lockup_drawn`): the
//!   mark pops, ULTIMATE tracks in, GUN GAME flickers on; 0.9-1.5 s the
//!   rule scans out under it, "Welcome to <app>" (`EscMenuTitle`) rises
//!   with its eyebrow and tagline (`[intro]` in the data); 1.3-1.9 s the
//!   system check types in (display, audio, games installed from the
//!   mount doctor's `LocalMounts`, account: none yet); the tick bar runs
//!   the whole length; 2.3-2.6 s it fades into Welcome, whose entry plays
//!   under the fade. Never blocking: any key or click ends it at once
//!   (`end_intro`), Welcome settled, the key going no further, and the
//!   field types on the next. `ui_motion_scale` stretches it; at 0 it is
//!   its last frame, still, for 0.6 s. A sting from the app's UI sounds
//!   (`pass_sweep`, `end_reveal`: the `ui` bus at its volume, silent
//!   without them).
//! - **Welcome** (`Step::Welcome`): the one field takes the whole hub
//!   link, a web address with the code, or the bare code
//!   (`chimera_assets::account::InviteInput`), checked with its hub by
//!   itself (`invite_pause`): at once on a paste, else `INVITE_PAUSE`
//!   after the last key, the screen staying (`Onboarding::auto`); a yes
//!   is kept for that text (`valid`) and Continue goes on with no other
//!   call (Continue while the check runs waits on Checking); the pane
//!   "What an invite carries" lights each part found. A link on the
//!   clipboard fills the field and is checked: at the start
//!   (`from_clipboard`), and whenever the window comes back to the front
//!   (`refocus_clipboard`) while the field is empty or holds something
//!   else that isn't a checked invite.
//!   A clicked invite link (`chimera_client::client::link_handler`: the
//!   game started by it, or the running game it was handed to;
//!   `open_links`) brings Welcome up with the field filled
//!   (`Source::Opened`) and checks it at once, the window asked to the
//!   front; over the recovery key it waits. `link_setup` (a real start)
//!   registers the links for this program and holds the inbox.
//!   "I already have an account" (`Step::Have`) and "Play without an
//!   account" (writes the skip mark) under it.
//! - **Checking**: `POST /accounts/check` on a thread (`OnboardingHub`);
//!   a wrong, used or withdrawn code, an unreachable hub and another
//!   certificate each say what happened, why and what to do (`Problem`),
//!   the input kept; the error's one helpful action is the hero (Try
//!   another clears the field, Retry, Back with "Copy details for the
//!   owner" for a certificate: no field, no way on). A device link code
//!   goes straight to redeeming.
//! - **Addresses hidden** (`chimera_ui::secret`, `REVEAL_HOST`): the
//!   hub's address shows masked in the field, the pane, Checking and Hub,
//!   with an eye key; masked again whenever a screen opens.
//! - **Hub**: "You're joining <name>", the hub's address, the certificate
//!   check, its players and its MOTD; "Not this one" goes back.
//! - **Name**: what you choose at the left, as a form (the kit's
//!   `form_field`: a title, a line on what it's for, the input): the
//!   username (the rules lit as typed,
//!   `chimera_assets::codes::UsernameRules`; availability asked of the
//!   hub 0.45 s after the last key, with free suggestions when taken), the
//!   display name (optional; `codes::display_name`'s rules as typed: an
//!   error holds Continue), the avatar beside them (`cl_avatar`'s 32
//!   looks in tiles as big as the room allows; arrows move it while it has
//!   the focus); at the right only the preview of how others see you.
//!   Continue redeems: the device key is made here.
//! - **Key**: the recovery key in groups, masked with an eye key
//!   (`REVEAL_KEY`: streamers), Copy (Ctrl+C) and Save as file
//!   (`OnboardingFiles::key_dir`) never needing it shown; a saved file's
//!   whole path shows under them (`SavedPath`) with Copy path; "I saved
//!   it" (Space) unlocks Continue.
//! - **Have**: Link a device (a link code in six cells, where it comes
//!   from drawn small at the right) or Recover with key (Q and E);
//!   the hub's link asked when this PC knows none. A recovery shows the
//!   new key (the old one stops working).
//! - **Done** (`finish`): the account file written and made live, the
//!   avatar set (`cl_avatar`), the hub and parties of the account taken
//!   up, and a toast on the main menu ("Welcome, <name>").
//! - **No blocking animations** (decision 8) and no entrance slide (the
//!   kit's rule, `chimera_ui::shell`): each screen appears whole; only the
//!   intro and the controls' own small motions move.
//! - The menus behind it take no keys while it shows (`takes_keys`).
//! - **Returning players** see no sign-in screen: the account's tickets
//!   come silently (`chimera_assets::account::keep_fresh`, the hub's
//!   `use_user_hub`). The main menu shows a **hub indicator** under the
//!   bar's rack (`show_chip`: a light, green while the hub answers, amber
//!   when its last call failed, and the hub's name, "Offline · <hub>"
//!   then; the address never), and while the hub can't be reached a slim
//!   **offline banner** ("Can't reach <hub>", playing offline goes on,
//!   Retry now; it retries by itself every `RETRY_SECS`). There is no
//!   account card (the user's report, 2026-11-22): your own avatar in the
//!   party strip opens Settings > Account (`account_page`), hovering any
//!   member's avatar previews them (`chimera_ui::shell::Tip::card`).
//!   `AccountShown` is the account's names (the app's start, or the first
//!   run's end), which the strip's preview of you uses
//!   (`chimera_client::client::profile::AccountNames`).
//! - `ui_scene onboarding intro [t]` plays the intro (held still at `t`
//!   seconds when given, for screenshots); `ui_scene onboarding <welcome|clipboard|checked|checking|used|unknown|
//!   unreachable|certificate|hub|name|name-bad|name-taken|key|key-saved|have|
//!   recover|done|returning|offline|off>` stages each screen with a made-up hub
//!   (`StagedHub`), for screenshots.
//!
//! Known shortcuts: the intro's timings are numbers in the code (its
//! words are data); the screens rebuild whole on any
//! change; no keyboard focus between the buttons (Enter is the hero,
//! Esc goes back).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};
use chimera_assets::{
    account::{self, AccountFile, CheckAnswer, Failure, HubLink, HubPreview, InputKind, InviteInput, NameCheck, RedeemRequest},
    codes::{self, UsernameRules},
};
use chimera_ui::{
    Role, Tone, UiStyle, secret,
    avatar::{AvatarLook, AvatarMarks, Face},
    controls::{Kind, NOTE, SENTENCE, Size, State, Ui},
    form_page::{FieldSpec, FieldStatus, SecretSpec},
    motion::{Ease, Motion, Prop, Timeline, Track, UiClock},
    text_edit::{Caret, TextKey, text_key},
};

use crate::main_menu::MenuUiCamera;

/// Over the main menu and Settings, under the party panel, the report
/// dialog and the console.
pub const ONBOARDING_Z: i32 = 93;
/// Seconds between the offline banner's own retries.
pub const RETRY_SECS: f64 = 30.0;
/// The screens' words.
pub const ONBOARDING_TOML: chimera_core::game_data::GameData = chimera_core::game_data!("data/onboarding.toml");
/// Seconds after the last key before a username is asked of the hub.
pub const NAME_PAUSE: f64 = 0.45;

/// Why a typed display name can't be had (`codes::display_name`), in a
/// player's words; none when empty (the username then) or fine.
pub fn display_problem(display: &str) -> Option<String> {
    if display.trim().is_empty() {
        return None;
    }
    codes::display_name(display).err().map(|e| {
        let mut c = e.chars();
        c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect::<String>() + ".")
    })
}
/// Seconds the welcome toast stays on the main menu.
pub const TOAST_SECS: f64 = 6.0;
/// Seconds after the last key before the invite field is checked with
/// its hub (a paste checks at once).
pub const INVITE_PAUSE: f64 = 0.6;
/// The reveal keys (`chimera_ui::secret`): the hub's address, the
/// recovery key.
pub const REVEAL_HOST: &str = "onboarding.host";
pub const REVEAL_KEY: &str = "onboarding.key";

// ---------------------------------------------------------------------------
// The words.

/// `data/onboarding.toml`: `t("welcome.title")`; a missing key shows as
/// `[welcome.title]` (a test looks for those).
#[derive(Resource, Clone, Debug)]
pub struct Words(toml::Table);

impl Default for Words {
    fn default() -> Self {
        Words(toml::from_str(&ONBOARDING_TOML).expect("data/onboarding.toml parses"))
    }
}

impl Words {
    fn get(&self, key: &str) -> Option<&toml::Value> {
        let mut v: Option<&toml::Value> = None;
        for part in key.split('.') {
            v = match v {
                None => self.0.get(part),
                Some(t) => t.get(part),
            };
            v?;
        }
        v
    }

    pub fn t(&self, key: &str) -> String {
        self.get(key).and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("[{key}]"))
    }

    /// `t` with `{hub}`, `{host}`, `{name}` and `{user}` filled in.
    pub fn f(&self, key: &str, fill: &[(&str, &str)]) -> String {
        let mut s = self.t(key);
        for (k, v) in fill {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    pub fn list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec![format!("[{key}]")])
    }

    fn at(&self, key: &str, i: usize) -> String {
        self.list(key).get(i).cloned().unwrap_or_else(|| format!("[{key}.{i}]"))
    }
}

// ---------------------------------------------------------------------------
// The model.

/// The screen showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Welcome,
    Checking,
    Hub,
    Name,
    Key,
    Have,
}

/// What went wrong, told on the screen it happened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Not a link, a web address or a code.
    NotInvite,
    /// A bare code and no hub known.
    NeedsHub,
    Used,
    Unknown,
    /// Revoked or expired.
    Withdrawn,
    /// Too many wrong codes from here (429).
    Busy,
    Unreachable,
    Certificate { offered: Option<String> },
    /// The hub's or the network's own words.
    Other(String),
}

impl Problem {
    /// A call's error as the screens tell it.
    pub fn of(error: &str) -> Problem {
        match Failure::of(error) {
            Failure::Unreachable(_) => Problem::Unreachable,
            Failure::Certificate { offered } => Problem::Certificate { offered },
            Failure::Refused { status: 404, .. } => Problem::Unknown,
            Failure::Refused { status: 429, .. } => Problem::Busy,
            Failure::Refused { status: 410, message } if message.contains("used") => Problem::Used,
            Failure::Refused { status: 410, .. } => Problem::Withdrawn,
            Failure::Refused { message, .. } => Problem::Other(message),
            Failure::Other(m) => Problem::Other(m),
        }
    }
}

/// Where the welcome field's text came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Typed,
    Clipboard,
    /// An invite link opened the game.
    Opened,
}

/// The name screen's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameField {
    #[default]
    Username,
    Display,
    Avatar,
}

/// "I already have an account"'s tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HaveTab {
    #[default]
    Link,
    Recover,
}

/// The first run's state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Onboarding {
    pub active: bool,
    pub step: Step,
    /// When the screen came up (`UiClock`): its entry plays from here.
    pub since: f64,
    pub input: String,
    pub caret: Caret,
    pub source: Source,
    /// The field as last read (a check, or Enter).
    pub parsed: Option<InviteInput>,
    /// When the field last changed (`UiClock`): checked `INVITE_PAUSE`
    /// after it.
    pub input_at: f64,
    /// Check the field at the next frame (a paste, a clipboard fill).
    pub check_now: bool,
    /// The field's own check under way (its call number): the screen stays.
    pub auto: Option<u32>,
    /// The field's text the hub answered for (`answer` is its answer):
    /// Continue goes on with no other call.
    pub valid: Option<String>,
    /// The hub this PC used last (a bare code's).
    pub last_hub: Option<HubLink>,
    /// The hub being joined and the code.
    pub link: Option<HubLink>,
    pub problem: Option<Problem>,
    pub answer: Option<CheckAnswer>,
    pub username: String,
    pub display: String,
    pub field: NameField,
    pub ucaret: Caret,
    pub dcaret: Caret,
    /// `cl_avatar`'s look byte.
    pub avatar: u8,
    /// The hub's answer for `username`, and when it was last typed.
    pub name: Option<NameCheck>,
    pub typed_at: f64,
    pub asked_name: Option<String>,
    pub recovery: Option<String>,
    pub saved: bool,
    pub said: Option<String>,
    /// Where "Save as file" put the key (its whole path).
    pub saved_path: Option<String>,
    pub signed_in: Option<AccountFile>,
    pub tab: HaveTab,
    pub have_hub: String,
    pub have_code: String,
    pub have_on_hub: bool,
    /// A call under way (its number: a late answer to a cancelled one is
    /// dropped).
    pub pending: Option<u32>,
    pub asked: u32,
    /// The main menu's toast after the last screen.
    pub toast: Option<(String, String, f64)>,
    /// The first-launch intro playing over Welcome (`Intro`).
    pub intro: Option<Intro>,
}

/// The first-launch intro under way (module doc, "Intro").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intro {
    /// When it started (`UiClock`).
    pub start: f64,
    /// Held still this many seconds in (`ui_scene onboarding intro <t>`):
    /// it never ends by itself.
    pub held: Option<f32>,
    /// Waiting for the app to be warm (`chimera_ui::motion::Warmth`): its
    /// first frame stands still until then, and its clock starts there.
    pub warming: bool,
}

impl Intro {
    /// Live from `now`.
    pub fn live(now: f64) -> Self {
        Intro { start: now, held: None, warming: false }
    }

    /// Where it stands still, if it does: held, or at 0 while warming.
    pub fn still_at(&self) -> Option<f32> {
        self.held.or(self.warming.then_some(0.0))
    }
}

impl Onboarding {
    fn go(&mut self, step: Step, now: f64) {
        self.step = step;
        self.since = now;
        self.problem = None;
    }

    /// Whether the name on the screen can be had (the hub said so for it).
    pub fn name_ok(&self) -> bool {
        self.name.as_ref().is_some_and(|n| n.available && n.username == self.username.trim().to_ascii_lowercase())
    }

    fn hub_name(&self) -> String {
        self.answer.as_ref().map(|a| a.hub.name.clone()).unwrap_or_else(|| "the hub".into())
    }

    fn host(&self) -> String {
        self.link.as_ref().map(|l| l.host.clone()).unwrap_or_default()
    }
}

/// Whether the first run has the keys (the menus behind it take none).
pub fn takes_keys(o: Option<&Onboarding>) -> bool {
    o.is_some_and(|o| o.active)
}

/// Where the first run keeps what it makes. None: nothing written (tests,
/// staged scenes).
#[derive(Resource, Clone, Debug, Default)]
pub struct OnboardingFiles {
    pub account: Option<PathBuf>,
    /// Written by "Play without an account": the first run isn't offered
    /// again.
    pub skip_mark: Option<PathBuf>,
    /// Where "Save as file" puts the recovery key.
    pub key_dir: Option<PathBuf>,
    /// Written when the first-launch intro ends or is skipped: it plays
    /// once.
    pub intro_mark: Option<PathBuf>,
}

impl OnboardingFiles {
    /// The user's (beside `account.toml`; the key in Documents).
    pub fn user() -> Self {
        let account = AccountFile::path();
        let skip_mark = account.as_ref().map(|p| p.with_file_name("onboarding-skipped"));
        let intro_mark = account.as_ref().map(|p| p.with_file_name("onboarding-intro-seen"));
        let key_dir = dirs::document_dir().or_else(|| account.as_ref().and_then(|p| p.parent().map(PathBuf::from)));
        Self { account, skip_mark, key_dir, intro_mark }
    }
}

// ---------------------------------------------------------------------------
// The hub.

/// What the first run asks of a hub (the network's, or a staged one).
pub trait OnboardingHub: Send + Sync {
    fn check(&self, link: &HubLink, code: &str, username: Option<&str>) -> Result<CheckAnswer, String>;
    fn redeem(&self, link: &HubLink, request: RedeemRequest) -> Result<(AccountFile, Option<String>), String>;
    fn recover(&self, link: &HubLink, key: &str) -> Result<(AccountFile, Option<String>), String>;
}

/// The real hub (`chimera_assets::account`).
pub struct NetworkHub;

impl OnboardingHub for NetworkHub {
    fn check(&self, link: &HubLink, code: &str, username: Option<&str>) -> Result<CheckAnswer, String> {
        account::check(link, code, username)
    }
    fn redeem(&self, link: &HubLink, request: RedeemRequest) -> Result<(AccountFile, Option<String>), String> {
        account::redeem_request(link, request)
    }
    fn recover(&self, link: &HubLink, key: &str) -> Result<(AccountFile, Option<String>), String> {
        account::recover(link, "", key, &account::device_label())
    }
}

/// A made-up hub (`ui_scene onboarding`, tests): "Friday Night Hub";
/// `AAA-AAA-AAA` used, `BBB-BBB-BBB` unknown, `CCC-CCC-CCC` withdrawn,
/// `RRR-RRR-RRR` unreachable, `CER-T00-000` another certificate; the
/// names in `taken` are taken; any other code is a fresh invite.
#[derive(Clone, Debug)]
pub struct StagedHub {
    pub taken: Vec<String>,
}

impl Default for StagedHub {
    fn default() -> Self {
        StagedHub { taken: ["rocket", "sam", "player", "rocket2"].map(String::from).to_vec() }
    }
}

/// The recovery key the staged hub hands out (made up).
pub const STAGED_KEY: &str = "RK7F-2M9Q-XA4T-8WLC";

impl StagedHub {
    fn answer(&self, kind: &str) -> CheckAnswer {
        CheckAnswer {
            kind: kind.into(),
            hub: HubPreview {
                name: "Friday Night Hub".into(),
                motd: "Gun game every Friday from 8 pm. Maps rotate weekly; vote in the lobby. Be nice, and say hi in chat.".into(),
                players: 14,
            },
            account: (kind != "invite").then(|| "sam_rocket".into()),
            username: None,
        }
    }

    fn file(&self, username: &str, display: Option<String>) -> AccountFile {
        AccountFile {
            url: "https://hub.example.net:27080".into(),
            account: "00".repeat(16),
            device: "11".repeat(16),
            display_name: display.unwrap_or_else(|| username.to_string()),
            username: username.into(),
            device_key: "22".repeat(32),
            ..Default::default()
        }
    }
}

impl OnboardingHub for StagedHub {
    fn check(&self, _: &HubLink, code: &str, username: Option<&str>) -> Result<CheckAnswer, String> {
        let n = codes::normalize(code).unwrap_or_default();
        match n.as_str() {
            "AAAAAAAAA" => return Err("the hub refused: this invite was used already (410)".into()),
            "BBBBBBBBB" => return Err("the hub refused: unknown code (404)".into()),
            "CCCCCCCCC" => return Err("the hub refused: this invite was revoked (410)".into()),
            "RRRRRRRRR" => return Err("the hub at https://hub.example.net:27080 didn't answer (Connection refused)".into()),
            "CERT00000" => {
                return Err("the hub at https://hub.example.net:27080 didn't answer (io: the asset service's certificate isn't the \
                            one the game server named (fingerprint 3f9a0b71c2d45e6f expected, 77d0e2c4a51b19af offered))"
                    .into());
            }
            _ => {}
        }
        let mut a = self.answer(if n.len() == codes::LINK_LEN { "link" } else { "invite" });
        a.username = username.map(|u| {
            let folded = u.trim().to_ascii_lowercase();
            let taken = |x: &str| self.taken.iter().any(|t| t == x);
            match codes::choose_username(&folded) {
                Err(p) => NameCheck { username: folded, available: false, problem: Some(p), suggestions: vec![] },
                Ok(u) if taken(&u) => NameCheck {
                    suggestions: codes::suggestions(&u, 3, taken),
                    username: u,
                    available: false,
                    problem: Some("taken".into()),
                },
                Ok(u) => NameCheck { username: u, available: true, problem: None, suggestions: vec![] },
            }
        });
        Ok(a)
    }

    fn redeem(&self, link: &HubLink, request: RedeemRequest) -> Result<(AccountFile, Option<String>), String> {
        let a = self.check(link, &request.code, request.username.as_deref())?;
        if a.kind == "link" {
            return Ok((self.file("sam_rocket", Some("Sam Rocket".into())), None));
        }
        let u = a.username.ok_or("the hub refused: choose a username (400)")?;
        if !u.available {
            return Err(format!("the hub refused: the username {:?} is taken (409)", u.username));
        }
        Ok((self.file(&u.username, request.display_name.filter(|d| !d.trim().is_empty())), Some(STAGED_KEY.into())))
    }

    fn recover(&self, _: &HubLink, key: &str) -> Result<(AccountFile, Option<String>), String> {
        if codes::normalize(key).is_some_and(|k| k.len() == codes::RECOVERY_LEN) {
            Ok((self.file("sam_rocket", Some("Sam Rocket".into())), Some(STAGED_KEY.into())))
        } else {
            Err("the hub refused: that recovery key doesn't match an account (403)".into())
        }
    }
}

/// The hub the first run asks; `inline` answers in the same frame
/// (tests, staged scenes), else on a thread.
#[derive(Resource, Clone)]
pub struct OnboardingBackend {
    pub hub: Arc<dyn OnboardingHub>,
    pub inline: bool,
}

impl Default for OnboardingBackend {
    fn default() -> Self {
        OnboardingBackend { hub: Arc::new(NetworkHub), inline: false }
    }
}

impl OnboardingBackend {
    pub fn staged() -> Self {
        OnboardingBackend { hub: Arc::new(StagedHub::default()), inline: true }
    }
}

/// A call to the hub.
#[derive(Clone, Debug)]
enum Ask {
    Check { link: HubLink, code: String, username: Option<String> },
    Redeem { link: HubLink, request: RedeemRequest },
    Recover { link: HubLink, key: String },
}

/// Its answer.
#[derive(Clone, Debug)]
enum Reply {
    Checked(Result<CheckAnswer, String>, Option<String>),
    SignedIn(Result<(AccountFile, Option<String>), String>),
}

/// Answers waiting for the next frame (by call number).
#[derive(Resource, Clone, Default)]
struct Answers(Arc<Mutex<Vec<(u32, Reply)>>>);

fn ask(o: &mut Onboarding, backend: &OnboardingBackend, answers: &Answers, what: Ask, quiet: bool) {
    o.asked += 1;
    let n = o.asked;
    if !quiet {
        o.pending = Some(n);
    }
    let hub = backend.hub.clone();
    let out = answers.0.clone();
    let run = move || {
        let reply = match what {
            Ask::Check { link, code, username } => {
                let r = hub.check(&link, &code, username.as_deref());
                Reply::Checked(r, username)
            }
            Ask::Redeem { link, request } => Reply::SignedIn(hub.redeem(&link, request)),
            Ask::Recover { link, key } => Reply::SignedIn(hub.recover(&link, &key)),
        };
        out.lock().unwrap_or_else(|e| e.into_inner()).push((n, reply));
    };
    if backend.inline {
        run();
    } else {
        std::thread::spawn(run);
    }
}

// ---------------------------------------------------------------------------
// The plugin.

/// A click on one of the screens.
#[derive(Component, Message, Clone, Debug, PartialEq)]
pub enum ObAction {
    Continue,
    Have,
    Skip,
    Back,
    Retry,
    CopyDetails,
    Focus(NameField),
    Avatar(u8),
    Suggest(String),
    Copy,
    Save,
    Saved,
    Tab(HaveTab),
    HaveField(bool),
    Clear,
    /// Copies where "Save as file" put the key.
    CopyPath,
    /// Opens the folder "Save as file" wrote into (`FolderShown` when
    /// staged: nothing is opened).
    ShowFolder,
}

#[derive(Component)]
pub struct OnboardingRoot;

/// The folder Show in folder opened last (a staged run records it and
/// opens nothing).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FolderShown(pub Option<std::path::PathBuf>);

/// Opens the system's file manager at `file`, selected where the system
/// can (Windows' Explorer, macOS's Finder), else its folder.
pub fn show_in_folder(file: &std::path::Path) {
    let r = if cfg!(windows) {
        std::process::Command::new("explorer").arg(format!("/select,{}", file.display())).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(file).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(file.parent().unwrap_or(file)).spawn()
    };
    if let Err(e) = r {
        warn!("show in folder: {e}");
    }
}

/// The recovery key file's path on the key screen (after Save as file).
#[derive(Component)]
pub struct SavedPath;

/// The bar's lockup (the game's own logo, `chimera_ui::controls::Ui::
/// lockup_drawn`).
#[derive(Component)]
pub struct OnboardingLockup;

#[derive(Component)]
pub struct ToastRoot;

#[derive(Component)]
pub struct ChipRoot;

/// The signed-in account's names for the chip (None: no account).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountShown(pub Option<Shown>);

/// What the chip says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub display: String,
    pub username: String,
    /// The hub's name when its document isn't at hand.
    pub hub: String,
}

impl Shown {
    pub fn of(file: &AccountFile, hub: &str) -> Self {
        let display = if file.display_name.is_empty() { file.username.clone() } else { file.display_name.clone() };
        Shown { display, username: file.username.clone(), hub: hub.to_string() }
    }
}

/// The banner's Retry now.
#[derive(Component, Clone, Copy, Debug)]
pub struct RetryHub;

pub struct OnboardingPlugin;

impl Plugin for OnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Onboarding>()
            .init_resource::<Words>()
            .init_resource::<OnboardingFiles>()
            .init_resource::<OnboardingBackend>()
            .init_resource::<Answers>()
            .init_resource::<AccountShown>()
            .add_message::<ObAction>();
        if !app.world().contains_resource::<Messages<KeyboardInput>>() {
            app.add_message::<KeyboardInput>();
        }
        if !app.world().contains_resource::<Messages<bevy::window::WindowFocused>>() {
            app.add_message::<bevy::window::WindowFocused>();
        }
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "onboarding", scene);
        let _ = chimera_core::console::ConsoleAppExt::console_command(
            app,
            "onboarding",
            "onboarding [0|1]: show the first run's screens (or hide them).",
            |w, a| {
                let on = a.first().is_none_or(|v| v != "0");
                let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
                let mut o = w.resource_mut::<Onboarding>();
                o.active = on;
                o.go(Step::Welcome, now);
                Ok(None)
            },
        );
        app.add_systems(Update, (from_clipboard, refocus_clipboard, open_links, clicks, keys, intro_tick, invite_pause, poll, name_pause, act, show, show_intro, show_toast).chain())
            .init_resource::<chimera_client::client::profile::AccountNames>()
            .add_systems(Update, (retry_hub, share_names, show_chip).chain())
            .add_systems(Update, (hold_intro, intro_clock).after(show_intro));
    }
}

/// The app's start (a real one: `main_menu`'s `keep_recent`): invite
/// links registered for this program under the app's name, and the
/// running game's inbox held (`chimera_client::client::link_handler`).
pub fn link_setup(mut commands: Commands, title: Option<Res<crate::esc_menu::EscMenuTitle>>) {
    let name = title.map_or_else(|| crate::esc_menu::EscMenuTitle::default().0, |t| t.0.clone());
    commands.insert_resource(chimera_client::client::link_handler::LinkSetup::user(&name));
}

/// An invite link that opened the game, or was handed to it running
/// (`link_handler::OpenedLinks`, each already a well-formed invite): the
/// first run comes up on Welcome with the field filled and checks it at
/// once (as Enter), the window asked to the front. Not over the recovery
/// key (it shows once): the link waits until that screen is left.
pub fn open_links(
    links: Option<ResMut<chimera_client::client::link_handler::OpenedLinks>>,
    mut o: ResMut<Onboarding>,
    clock: Res<UiClock>,
    mut out: MessageWriter<ObAction>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Some(mut links) = links else { return };
    if links.0.is_empty() || (o.active && o.step == Step::Key) {
        return;
    }
    // The latest wins.
    let Some(link) = links.0.drain(..).next_back() else { return };
    let Some(parsed) = InviteInput::parse(&link) else { return };
    let now = clock.now;
    o.active = true;
    o.pending = None;
    o.go(Step::Welcome, now);
    o.input = link;
    o.caret = Caret::end_of(&o.input);
    o.source = Source::Opened;
    o.parsed = Some(parsed);
    o.auto = None;
    o.valid = None;
    out.write(ObAction::Continue);
    for mut w in &mut windows {
        if !w.focused {
            w.focused = true;
        }
    }
}

/// Whether the first run is due with `files`: no account file and no
/// skip mark (a token for an asset service with no hub doesn't count).
pub fn first_run_due(files: &OnboardingFiles) -> bool {
    let signed_in = files.account.as_ref().is_some_and(|p| matches!(AccountFile::load_from(p), Ok(Some(_))));
    let skipped = files.skip_mark.as_ref().is_some_and(|p| p.exists());
    !signed_in && !skipped
}

/// The app's start: the first run when this PC has no account and no skip
/// mark (module doc).
pub fn maybe_start(
    mut o: ResMut<Onboarding>,
    mut files: ResMut<OnboardingFiles>,
    mut shown: ResMut<AccountShown>,
    clock: Option<Res<UiClock>>,
    motion: Option<Res<Motion>>,
) {
    *files = OnboardingFiles::user();
    if let Some(file) = files.account.as_ref().and_then(|p| AccountFile::load_from(p).ok().flatten()) {
        shown.0 = Some(Shown::of(&file, "the hub"));
    }
    start_first_run(&mut o, &files, clock.map_or(0.0, |c| c.now), motion.map_or(1.0, |m| m.scale));
}

/// The first run at `now` when it is due with `files`, the intro first
/// when that is due too (`intro_due`); false when nothing is due.
pub fn start_first_run(o: &mut Onboarding, files: &OnboardingFiles, now: f64, scale: f32) -> bool {
    if !first_run_due(files) {
        return false;
    }
    o.active = true;
    o.go(Step::Welcome, now);
    o.avatar = rand_avatar();
    if intro_due(files) {
        // Its clock starts once the app is warm (`intro_tick`).
        o.intro = Some(Intro { warming: true, ..Intro::live(now) });
        o.since = now + welcome_at(scale);
    }
    true
}

/// Whether the first-launch intro plays: the first run is due and the
/// intro's mark isn't there (it played, or was skipped, once already).
pub fn intro_due(files: &OnboardingFiles) -> bool {
    first_run_due(files) && !files.intro_mark.as_ref().is_some_and(|p| p.exists())
}

fn rand_avatar() -> u8 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    (t / 1000 % 32) as u8
}

fn clicks(pressed: Query<(&Interaction, &ObAction), Changed<Interaction>>, mut out: MessageWriter<ObAction>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(a.clone());
        }
    }
}

/// The keys: typing into the screen's field, Enter, Esc, Tab, Q and E,
/// Space, the avatar's arrows.
#[allow(clippy::too_many_arguments)]
fn keys(
    mut events: MessageReader<KeyboardInput>,
    buttons: Option<Res<ButtonInput<KeyCode>>>,
    mut o: ResMut<Onboarding>,
    mut out: MessageWriter<ObAction>,
    clock: Res<UiClock>,
    guard: Option<ResMut<chimera_ui::shell::BackGuard>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    (shared, mut own): (Option<ResMut<chimera_ui::clipboard::Clipboard>>, Local<chimera_ui::clipboard::Clipboard>),
    files: Res<OnboardingFiles>,
) {
    if !o.active || console.is_some_and(|c| c.open) {
        events.clear();
        return;
    }
    if let Some(mut g) = guard {
        g.hold();
    }
    // The intro: any key skips it to Welcome at once, and goes no further
    // (decision 8: never blocking).
    if o.intro.is_some() {
        if events.read().any(|k| k.state.is_pressed()) {
            end_intro(&mut o, &files, clock.now, true);
        }
        return;
    }
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let (ctrl, shift) = buttons.as_ref().map_or((false, false), |b| {
        (
            b.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]),
            b.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        )
    });
    let now = clock.now;
    for k in events.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match (&k.logical_key, o.step) {
            (Key::Enter, _) => {
                out.write(ObAction::Continue);
                continue;
            }
            (Key::Escape, Step::Welcome) => {
                out.write(ObAction::Clear);
                continue;
            }
            (Key::Escape, _) => {
                out.write(ObAction::Back);
                continue;
            }
            (Key::Tab, Step::Name) => {
                o.field = match (o.field, shift) {
                    (NameField::Username, false) | (NameField::Avatar, true) => NameField::Display,
                    (NameField::Display, false) | (NameField::Username, true) => NameField::Avatar,
                    _ => NameField::Username,
                };
                continue;
            }
            (Key::Tab, Step::Have) => {
                let on_hub = !o.have_on_hub && o.last_hub.is_none();
                o.have_on_hub = on_hub;
                continue;
            }
            (Key::Space, Step::Key) => {
                out.write(ObAction::Saved);
                continue;
            }
            (Key::Character(c), Step::Key) if ctrl && c.eq_ignore_ascii_case("c") => {
                out.write(ObAction::Copy);
                continue;
            }
            (Key::Character(c), Step::Key) if ctrl && c.eq_ignore_ascii_case("s") => {
                out.write(ObAction::Save);
                continue;
            }
            (Key::Character(c), Step::Have) if !ctrl && (c == "q" || c == "Q" || c == "e" || c == "E") && o.have_code.is_empty() => {
                out.write(ObAction::Tab(if c.eq_ignore_ascii_case("q") { HaveTab::Link } else { HaveTab::Recover }));
                continue;
            }
            (Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown, Step::Name) if o.field == NameField::Avatar => {
                let d: i16 = match k.logical_key {
                    Key::ArrowLeft => -1,
                    Key::ArrowRight => 1,
                    Key::ArrowUp => -8,
                    _ => 8,
                };
                o.avatar = (o.avatar as i16 + d).rem_euclid(32) as u8;
                continue;
            }
            _ => {}
        }
        let Some(tk) = text_key(&k.logical_key, k.key_code, ctrl, shift) else { continue };
        let o = &mut *o;
        match o.step {
            Step::Welcome => {
                let mut caret = o.caret;
                let paste = matches!(tk, TextKey::Paste);
                if caret.key(&mut o.input, tk, clipboard, 512, false) {
                    edited(o, now);
                    // A paste is checked at once; typing after a pause.
                    o.check_now = paste;
                }
                o.caret = caret;
            }
            Step::Name => match o.field {
                NameField::Username => {
                    let mut caret = o.ucaret;
                    if caret.key(&mut o.username, tk, clipboard, codes::NAME_MAX, false) {
                        o.typed_at = now;
                        o.problem = None;
                    }
                    o.ucaret = caret;
                }
                NameField::Display => {
                    let mut caret = o.dcaret;
                    caret.key(&mut o.display, tk, clipboard, codes::NAME_MAX, false);
                    o.dcaret = caret;
                }
                NameField::Avatar => {}
            },
            Step::Have => {
                let mut caret = Caret::end_of(if o.have_on_hub { &o.have_hub } else { &o.have_code });
                let text = if o.have_on_hub { &mut o.have_hub } else { &mut o.have_code };
                if caret.key(text, tk, clipboard, 512, false) {
                    o.problem = None;
                }
            }
            _ => {}
        }
    }
}

/// The invite field changed (`now`): what was read of it is dropped and
/// a check of it under way is forgotten.
fn edited(o: &mut Onboarding, now: f64) {
    o.parsed = None;
    o.problem = None;
    o.source = Source::Typed;
    o.input_at = now;
    o.auto = None;
    o.valid = None;
}

/// Checks the invite field with its hub once the player paused
/// (`INVITE_PAUSE`) or pasted, the screen staying on Welcome: no Enter or
/// Continue is needed to check it (the user's review, 2026-11-21).
fn invite_pause(mut o: ResMut<Onboarding>, backend: Res<OnboardingBackend>, answers: Res<Answers>, clock: Res<UiClock>) {
    if !o.active || o.step != Step::Welcome || o.intro.is_some() || o.pending.is_some() || o.auto.is_some() || o.problem.is_some() {
        return;
    }
    let text = o.input.trim().to_string();
    if text.is_empty() || o.valid.as_deref() == Some(text.as_str()) || (!o.check_now && clock.now - o.input_at < INVITE_PAUSE) {
        return;
    }
    o.check_now = false;
    let parsed = InviteInput::parse(&text);
    o.parsed = parsed.clone();
    let Some(p) = parsed else {
        o.problem = Some(Problem::NotInvite);
        return;
    };
    let Some(link) = p.link(o.last_hub.as_ref()) else {
        o.problem = Some(Problem::NeedsHub);
        return;
    };
    let Some(code) = link.code.clone() else {
        o.problem = Some(Problem::NotInvite);
        return;
    };
    o.link = Some(link.clone());
    ask(&mut o, &backend, &answers, Ask::Check { link, code, username: None }, true);
    o.auto = Some(o.asked);
}

/// The hub said yes to the field (`a`): an invite goes on to the hub's
/// screen; a link code or a re-invite is an account already, redeemed at
/// once.
fn go_on(o: &mut Onboarding, a: CheckAnswer, backend: &OnboardingBackend, answers: &Answers, now: f64) {
    let invite = a.kind == "invite";
    o.answer = Some(a);
    if invite {
        o.go(Step::Hub, now);
    } else if let (Some(link), Some(code)) = (o.link.clone(), o.link.as_ref().and_then(|l| l.code.clone())) {
        let request = RedeemRequest { code, device_label: account::device_label(), ..Default::default() };
        ask(o, backend, answers, Ask::Redeem { link, request }, false);
    }
}

/// The hub's answers.
fn poll(answers: Res<Answers>, mut o: ResMut<Onboarding>, backend: Res<OnboardingBackend>, clock: Res<UiClock>) {
    let got: Vec<(u32, Reply)> = std::mem::take(&mut *answers.0.lock().unwrap_or_else(|e| e.into_inner()));
    let now = clock.now;
    for (n, reply) in got {
        let ours = o.pending == Some(n);
        match reply {
            Reply::Checked(r, Some(name)) => {
                // A username's availability (quiet: the screen goes on).
                if o.step == Step::Name && name.trim().eq_ignore_ascii_case(o.username.trim()) {
                    match r {
                        Ok(a) => o.name = a.username,
                        Err(e) => o.problem = Some(Problem::of(&e)),
                    }
                }
            }
            Reply::Checked(r, None) if o.auto == Some(n) => {
                // The field's own check: the screen stays, Continue goes on.
                o.auto = None;
                if o.step == Step::Welcome {
                    match r {
                        Ok(a) => {
                            o.valid = Some(o.input.trim().to_string());
                            o.answer = Some(a);
                        }
                        Err(e) => o.problem = Some(Problem::of(&e)),
                    }
                }
            }
            Reply::Checked(r, None) if ours => {
                o.pending = None;
                match r {
                    Ok(a) => {
                        o.valid = Some(o.input.trim().to_string());
                        go_on(&mut o, a, &backend, &answers, now);
                    }
                    Err(e) => {
                        let back = if o.step == Step::Have { Step::Have } else { Step::Welcome };
                        o.step = back;
                        o.problem = Some(Problem::of(&e));
                    }
                }
            }
            Reply::SignedIn(r) if ours => {
                o.pending = None;
                match r {
                    Ok((file, key)) => {
                        o.signed_in = Some(file);
                        match key {
                            Some(k) => {
                                o.recovery = Some(k);
                                o.saved = false;
                                o.said = None;
                                o.go(Step::Key, now);
                            }
                            None => {
                                // A linked device: done.
                                o.step = Step::Key;
                                o.saved = true;
                            }
                        }
                    }
                    Err(e) => {
                        let p = Problem::of(&e);
                        if o.step == Step::Name && matches!(&p, Problem::Other(m) if m.contains("taken")) {
                            o.name = None;
                            o.asked_name = None;
                            o.typed_at = now - NAME_PAUSE;
                        } else if o.step == Step::Checking {
                            o.step = Step::Welcome;
                            o.problem = Some(p);
                        } else {
                            o.problem = Some(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // A linked device skips the key screen.
    if o.step == Step::Key && o.recovery.is_none() && o.signed_in.is_some() && o.pending.is_none() {
        o.saved = true;
    }
}

/// Asks the hub about the username once the player paused.
fn name_pause(mut o: ResMut<Onboarding>, backend: Res<OnboardingBackend>, answers: Res<Answers>, clock: Res<UiClock>) {
    if !o.active || o.step != Step::Name {
        return;
    }
    let name = o.username.trim().to_ascii_lowercase();
    if o.asked_name.as_deref() == Some(name.as_str()) || clock.now - o.typed_at < NAME_PAUSE {
        return;
    }
    o.asked_name = Some(name.clone());
    if !UsernameRules::of(&name).all() {
        o.name = None;
        return;
    }
    if let (Some(link), Some(code)) = (o.link.clone(), o.link.as_ref().and_then(|l| l.code.clone())) {
        ask(&mut o, &backend, &answers, Ask::Check { link, code, username: Some(name) }, true);
    }
}

/// What a click or key does.
#[allow(clippy::too_many_arguments)]
fn act(
    mut actions: MessageReader<ObAction>,
    mut o: ResMut<Onboarding>,
    backend: Res<OnboardingBackend>,
    answers: Res<Answers>,
    files: Res<OnboardingFiles>,
    words: Res<Words>,
    clock: Res<UiClock>,
    (shared, mut own): (Option<ResMut<chimera_ui::clipboard::Clipboard>>, Local<chimera_ui::clipboard::Clipboard>),
    mut commands: Commands,
) {
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let now = clock.now;
    for a in actions.read() {
        if !o.active {
            continue;
        }
        if o.pending.is_some() && !matches!(a, ObAction::Back) {
            continue;
        }
        match (a, o.step) {
            (ObAction::Clear, _) => {
                o.input.clear();
                o.caret = Caret::default();
                edited(&mut o, now);
                o.said = None;
            }
            (ObAction::CopyDetails, Step::Welcome) => {
                if let Some(Problem::Certificate { offered }) = o.problem.clone() {
                    let expected = o.link.as_ref().and_then(|l| l.fingerprint.clone()).unwrap_or_default();
                    let fill = [("host", o.host()), ("name", short_fp(&expected)), ("user", offered.as_deref().map(short_fp).unwrap_or_else(|| "?".into()))];
                    let fill: Vec<(&str, &str)> = fill.iter().map(|(k, v)| (*k, v.as_str())).collect();
                    clipboard.set(&words.f("errors.details", &fill));
                    o.said = Some(words.t("errors.details_copied"));
                }
            }
            // Enter on an error does its hero: a wrong, used or withdrawn
            // code clears the field for another; a certificate that isn't
            // the invite's has no way on (Esc goes back).
            (ObAction::Continue, Step::Welcome)
                if matches!(o.problem, Some(Problem::Used | Problem::Unknown | Problem::Withdrawn | Problem::Busy | Problem::Certificate { .. })) =>
            {
                if !matches!(o.problem, Some(Problem::Certificate { .. })) {
                    o.input.clear();
                    o.caret = Caret::default();
                    edited(&mut o, now);
                    o.said = None;
                }
            }
            (ObAction::Skip, _) => {
                if let Some(p) = &files.skip_mark {
                    if let Some(d) = p.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::write(p, "The first run was skipped: play without an account.\n");
                }
                o.active = false;
            }
            (ObAction::Have, _) => {
                o.go(Step::Have, now);
                o.have_on_hub = false;
            }
            // Checked already (as typed or pasted): on at once.
            (ObAction::Continue, Step::Welcome) if o.answer.is_some() && o.valid.as_deref() == Some(o.input.trim()) => {
                let a = o.answer.clone().unwrap_or_default();
                go_on(&mut o, a, &backend, &answers, now);
            }
            // Its check under way: the checking screen waits for it.
            (ObAction::Continue, Step::Welcome) if o.auto.is_some() => {
                o.pending = o.auto.take();
                o.go(Step::Checking, now);
            }
            (ObAction::Continue | ObAction::Retry, Step::Welcome) => {
                o.auto = None;
                let parsed = InviteInput::parse(&o.input);
                o.parsed = parsed.clone();
                let Some(p) = parsed else {
                    o.problem = Some(Problem::NotInvite);
                    continue;
                };
                let Some(link) = p.link(o.last_hub.as_ref()) else {
                    o.problem = Some(Problem::NeedsHub);
                    continue;
                };
                let Some(code) = link.code.clone() else {
                    o.problem = Some(Problem::NotInvite);
                    continue;
                };
                o.link = Some(link.clone());
                o.go(Step::Checking, now);
                ask(&mut o, &backend, &answers, Ask::Check { link, code, username: None }, false);
            }
            (ObAction::Back, Step::Checking) => {
                o.pending = None;
                o.go(Step::Welcome, now);
            }
            (ObAction::Back, Step::Have) => o.go(Step::Welcome, now),
            (ObAction::Back, Step::Hub) => o.go(Step::Welcome, now),
            (ObAction::Back, Step::Name) => o.go(Step::Hub, now),
            (ObAction::Continue, Step::Hub) => {
                o.go(Step::Name, now);
                o.field = NameField::Username;
            }
            (ObAction::Focus(f), _) => o.field = *f,
            (ObAction::Avatar(b), _) => {
                o.avatar = *b % 32;
                o.field = NameField::Avatar;
            }
            (ObAction::Suggest(s), _) => {
                o.username = s.clone();
                o.ucaret = Caret::end_of(s);
                o.typed_at = now - NAME_PAUSE;
                o.field = NameField::Username;
            }
            (ObAction::Continue, Step::Name) => {
                if !o.name_ok() {
                    continue;
                }
                if display_problem(&o.display).is_some() {
                    o.field = NameField::Display;
                    continue;
                }
                let (Some(link), Some(code)) = (o.link.clone(), o.link.as_ref().and_then(|l| l.code.clone())) else { continue };
                let display = o.display.trim().to_string();
                let request = RedeemRequest {
                    code,
                    username: Some(o.username.trim().to_ascii_lowercase()),
                    device_label: account::device_label(),
                    avatar: Some(o.avatar),
                    display_name: (!display.is_empty()).then_some(display),
                    ..Default::default()
                };
                ask(&mut o, &backend, &answers, Ask::Redeem { link, request }, false);
            }
            (ObAction::Copy, Step::Key) => {
                if let Some(k) = o.recovery.clone() {
                    clipboard.set(&k);
                    o.said = Some(words.t("key.copied"));
                }
            }
            (ObAction::Save, Step::Key) => {
                let (Some(k), Some(dir)) = (o.recovery.clone(), files.key_dir.clone()) else {
                    o.said = Some(words.t("key.copied"));
                    if let Some(k) = o.recovery.clone() {
                        clipboard.set(&k);
                    }
                    continue;
                };
                let user = o.signed_in.as_ref().map(|f| f.username.clone()).unwrap_or_default();
                let name = format!("{}-recovery-key.txt", slug(&o.hub_name()));
                let text = words.f("key.file", &[("user", &user), ("hub", &o.hub_name()), ("host", &o.host()), ("name", &k)]);
                let path = dir.join(&name);
                match chimera_assets::store::write_atomic(&path, text.as_bytes()) {
                    Ok(()) => {
                        chimera_assets::sign::restrict(&path);
                        o.said = Some(words.t("key.saved"));
                        o.saved_path = Some(std::path::absolute(&path).unwrap_or(path.clone()).display().to_string());
                    }
                    Err(e) => o.said = Some(format!("{}: {e}", path.display())),
                }
            }
            (ObAction::ShowFolder, Step::Key) => {
                if let Some(p) = o.saved_path.clone() {
                    let dir = std::path::Path::new(&p).parent().map(std::path::Path::to_path_buf).unwrap_or_default();
                    if backend.inline {
                        commands.insert_resource(FolderShown(Some(dir)));
                    } else {
                        show_in_folder(std::path::Path::new(&p));
                    }
                }
            }
            (ObAction::CopyPath, Step::Key) => {
                if let Some(p) = o.saved_path.clone() {
                    clipboard.set(&p);
                    o.said = Some(words.t("key.path_copied"));
                }
            }
            (ObAction::Saved, Step::Key) => {
                let s = !o.saved;
                o.saved = s;
            }
            (ObAction::Continue, Step::Key) => {
                if o.saved {
                    finish(&mut o, &files, &words, now, &mut commands);
                }
            }
            (ObAction::Tab(t), _) => {
                o.tab = *t;
                o.have_code.clear();
                o.problem = None;
            }
            (ObAction::HaveField(hub), _) => o.have_on_hub = *hub,
            (ObAction::Continue, Step::Have) => {
                let hub_text = o.have_hub.trim().to_string();
                let hub = if hub_text.is_empty() {
                    o.last_hub.clone()
                } else {
                    InviteInput::parse(&hub_text).and_then(|i| i.link(None)).or_else(|| HubLink::parse(&hub_text).ok())
                };
                let Some(mut link) = hub else {
                    o.problem = Some(Problem::NeedsHub);
                    continue;
                };
                match o.tab {
                    HaveTab::Link => {
                        let code = o.have_code.trim().to_string();
                        if InviteInput::parse(&code).is_none_or(|i| !i.is_link_code()) {
                            o.problem = Some(Problem::Unknown);
                            continue;
                        }
                        link.code = Some(code.clone());
                        o.link = Some(link.clone());
                        ask(&mut o, &backend, &answers, Ask::Check { link, code, username: None }, false);
                    }
                    HaveTab::Recover => {
                        let key = o.have_code.trim().to_string();
                        o.link = Some(link.clone());
                        ask(&mut o, &backend, &answers, Ask::Recover { link, key }, false);
                    }
                }
            }
            _ => {}
        }
    }
    // A linked device: straight to the menu.
    if o.active && o.step == Step::Key && o.recovery.is_none() && o.signed_in.is_some() && o.pending.is_none() {
        finish(&mut o, &files, &words, now, &mut commands);
    }
}

fn slug(name: &str) -> String {
    let s: String = name.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "hub".into() } else { s }
}

/// The last screen's end: the account kept and made live, the avatar set,
/// the account's hub and parties taken up, the toast.
fn finish(o: &mut Onboarding, files: &OnboardingFiles, words: &Words, now: f64, commands: &mut Commands) {
    let Some(file) = o.signed_in.clone() else { return };
    if let Some(path) = &files.account {
        if let Err(e) = file.save_to(path) {
            o.said = Some(e);
            return;
        }
        account::set_live(Some(Arc::new(account::Session::new(file.clone(), Some(path.clone())))));
        commands.queue(|w: &mut World| {
            w.remove_resource::<chimera_net::net::hub::Hub>();
            let _ = w.run_system_cached(chimera_net::net::hub::use_user_hub);
            let _ = w.run_system_cached(chimera_net::net::party::use_user_party);
        });
    }
    // The base files every game reads, up front, in the background
    // (decision 12; `base_download`).
    commands.queue(|w: &mut World| {
        if w.contains_resource::<crate::base_download::BaseDownload>() {
            crate::base_download::start_after_sign_in(w);
        }
    });
    let avatar = o.avatar;
    if o.recovery.is_some() || o.answer.as_ref().is_some_and(|a| a.kind == "invite") {
        commands.queue(move |w: &mut World| {
            if let Some(mut c) = w.get_resource_mut::<chimera_core::console::Console>() {
                c.submit(format!("cl_avatar {avatar}"));
            }
        });
    }
    let name = if file.display_name.is_empty() { file.username.clone() } else { file.display_name.clone() };
    let hub = o.hub_name();
    let chip = Shown::of(&file, &hub);
    commands.queue(move |w: &mut World| w.insert_resource(AccountShown(Some(chip))));
    o.toast = Some((words.f("done.toast", &[("name", &name)]), words.f("done.toast_note", &[("hub", &hub)]), now));
    o.active = false;
}

// ---------------------------------------------------------------------------
// Drawing.

type ScreenKey = (Onboarding, chimera_ui::secret::Reveals, (u32, u32), Option<Entity>);

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    o: Res<Onboarding>,
    words: Res<Words>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<OnboardingRoot>>,
    (theme, kit, units, reveals): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>, Option<Res<chimera_ui::secret::Reveals>>),
    (mut images, mut icons, mut ambients): (Option<ResMut<Assets<Image>>>, Option<ResMut<chimera_ui::icons::IconImages>>, Option<ResMut<Assets<chimera_ui::AmbientMaterial>>>),
    mut shown: Local<Option<ScreenKey>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let cam = camera.iter().next();
    let key = o.active.then(|| {
        let mut snap = o.clone();
        snap.toast = None;
        snap.intro = None;
        snap.typed_at = 0.0;
        snap.asked_name = None;
        snap.since = 0.0;
        snap.input_at = 0.0;
        snap.check_now = false;
        (snap, reveals.as_deref().cloned().unwrap_or_default(), ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), cam)
    });
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if key.is_some() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let none = chimera_ui::secret::Reveals::default();
        let mut d = Draw { ui: &ui, w: &words, o: &o, reveals: reveals.as_deref().unwrap_or(&none), images: images.as_deref_mut(), icons: icons.as_deref_mut() };
        let r = d.screen(&mut commands, ambients.as_deref_mut());
        if let Some(cam) = cam {
            commands.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *shown = key;
}

/// The main menu's toast after the first run.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_toast(
    mut o: ResMut<Onboarding>,
    clock: Res<UiClock>,
    menu: Option<Res<crate::main_menu::PartyMenu>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<ToastRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<(String, u32)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    if o.toast.as_ref().is_some_and(|t| clock.now - t.2 > TOAST_SECS) {
        o.toast = None;
    }
    let on_menu = menu.as_deref().is_none_or(|m| m.screen == crate::main_menu::Screen::MainMenu);
    let key = o.toast.as_ref().filter(|_| on_menu && !o.active).map(|t| (t.0.clone(), (units.height * 8.0) as u32));
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((title, note, _)) = o.toast.clone().filter(|_| key.is_some()) {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let r = commands
            .spawn((
                ToastRoot,
                GlobalZIndex(ONBOARDING_Z),
                UiStyle { gap: Some(1.4), pad: Some([0.0, 2.6, 0.0, 2.0].into()), h: Some(3.6), ..UiStyle::row() }.abs([Some(36.0), Some(9.4), None, None]),
                Node::default(),
                Pickable::IGNORE,
            ))
            .id();
        // The hero slab, both its ends on the slant.
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, ..default() },
            chimera_ui::style::UiSlant::new(Tone::Hero, 1.0),
            Pickable::IGNORE,
            ChildOf(r),
        ));
        ui.text(&mut commands, r, Role::DisplayBlack, 1.05, Tone::OnHero, &title);
        ui.sentence(&mut commands, r, 0.86, Tone::OnHero, &note);
        if let Some(cam) = camera.iter().next() {
            commands.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *shown = key;
}

/// The banner's Retry now, and its own retries while the hub is offline.
fn retry_hub(
    pressed: Query<&Interaction, (Changed<Interaction>, With<RetryHub>)>,
    hub: Option<ResMut<chimera_net::net::hub::Hub>>,
    clock: Res<UiClock>,
    mut last: Local<f64>,
) {
    let Some(mut hub) = hub else { return };
    let clicked = pressed.iter().any(|i| *i == Interaction::Pressed);
    if hub.signed_in() && hub.offline() && !hub.busy && (clicked || clock.now - *last > RETRY_SECS) {
        *last = clock.now;
        hub.fetched = None;
        hub.refresh_document();
    }
}

/// The account's names to the party strip's preview of you.
fn share_names(shown: Res<AccountShown>, mut names: ResMut<chimera_client::client::profile::AccountNames>) {
    let want = chimera_client::client::profile::AccountNames(shown.0.as_ref().map(|a| (a.username.clone(), a.display.clone())));
    if *names != want {
        *names = want;
    }
}

/// The hub indicator and the offline banner on the main menu (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_chip(
    shown_account: Res<AccountShown>,
    (o, page): (Res<Onboarding>, Option<Res<crate::account_page::AccountPage>>),
    hub: Option<Res<chimera_net::net::hub::Hub>>,
    menu: Option<Res<crate::main_menu::PartyMenu>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<ChipRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    words: Res<Words>,
    mut shown: Local<Option<(Shown, bool, String, (u32, u32), Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let on_menu = menu.as_deref().is_some_and(|m| m.screen == crate::main_menu::Screen::MainMenu && m.page == crate::main_menu::MenuPage::Main);
    let cam = camera.iter().next();
    let page_open = crate::account_page::takes_keys(page.as_deref());
    let key = shown_account.0.clone().filter(|_| on_menu && !o.active && !page_open && crate::esc_menu::signal(Some(theme))).map(|a| {
        let offline = hub.as_deref().is_some_and(|h| h.offline());
        let hub_name = hub.as_deref().filter(|h| h.doc.is_some()).map(|h| h.name()).unwrap_or_else(|| a.hub.clone());
        (a, offline, hub_name, ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), cam)
    });
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((a, offline, hub_name, _, cam)) = key.clone() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let c = &mut commands;
        let r = c
            .spawn((ChipRoot, GlobalZIndex(ONBOARDING_Z - 1), UiStyle::column().abs([Some(0.0), Some(0.0), None, None]).w_pct(100.0).h_pct(100.0), Node::default(), Pickable::IGNORE))
            .id();
        // The hub indicator, under the rack at the right: not pressable.
        let chip = ui.boxed(c, r, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([None, Some(8.2), Some(4.6), None]));
        ui.mark(c, chip, if offline { Tone::Warn } else { Tone::Good }, 0.7, 0.7);
        let fill = [("hub", hub_name.as_str()), ("user", a.username.as_str())];
        let line = words.f(if offline { "chip.offline" } else { "chip.online" }, &fill);
        ui.text_plain(c, chip, Role::Mono, 0.56, if offline { Tone::Warn } else { Tone::Fg3 }, &line.to_uppercase());
        if offline {
            // The banner: a warning's tint, its bar at the left, a faint
            // outline (the mockups' 09b), on ink so the backdrop stays behind.
            let b = ui.boxed(c, r, UiStyle { gap: Some(1.6), pad: Some([1.0, 1.6, 1.0, 2.0].into()), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([Some(36.0), Some(13.4), Some(4.6), None]).bg_a(Tone::Ink2, 0.94).outline(Tone::Warn, 0.22));
            c.entity(b).insert(bevy::ui::FocusPolicy::Block);
            ui.boxed(c, b, UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), None, None]).bg_a(Tone::Warn, 0.08));
            ui.boxed(c, b, UiStyle { w: Some(0.34), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), None, Some(0.0)]).bg(Tone::Warn));
            ui.mark(c, b, Tone::Warn, 0.7, 0.7);
            ui.text(c, b, Role::DisplayBold, 0.92, Tone::Warn, &words.f("chip.banner_title", &fill));
            let s = ui.boxed(c, b, UiStyle { grow: Some(1.0), ..UiStyle::column() });
            ui.paragraph(c, s, Role::Text, 0.86, Tone::Fg2, &words.t("chip.banner_body"));
            let retry = ui.button(c, b, Kind::Secondary, &words.t("chip.retry_now"), None, chimera_ui::controls::Size::Regular, State::NONE);
            c.entity(retry).insert(RetryHub);
        }
        if let Some(cam) = cam {
            c.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *shown = key;
}

struct Draw<'a, 'b> {
    ui: &'a Ui<'a>,
    w: &'a Words,
    o: &'a Onboarding,
    reveals: &'a chimera_ui::secret::Reveals,
    images: Option<&'b mut Assets<Image>>,
    icons: Option<&'b mut chimera_ui::icons::IconImages>,
}

impl Draw<'_, '_> {
    /// A layout number (`[layout.onboarding]`, the theme's), else `d`.
    fn lay(&self, key: &str, d: f32) -> f32 {
        self.ui.theme.layout_opt("onboarding", key).unwrap_or(d)
    }

    /// The left column at its place (`col_x`, `col_y` plus `dy`), `w` wide.
    fn col(&self, c: &mut Commands, root: Entity, dy: f32, w: f32) -> Entity {
        self.abs(c, root, self.lay("col_x", 8.4), self.lay("col_y", 11.6) + dy, w)
    }

    /// The right pane at its place (`pane_x` plus `dx`, `col_y` plus `dy`).
    fn pane(&self, c: &mut Commands, root: Entity, dx: f32, dy: f32, w: f32) -> Entity {
        self.abs(c, root, self.lay("pane_x", 58.0) + dx, self.lay("col_y", 11.6) + dy, w)
    }

    /// The blueprint plate (the panes that explain): a navy wash, a faint
    /// accent outline and the corner brackets.
    fn blueprint(&self, c: &mut Commands, parent: Entity, gap: f32) -> Entity {
        let bp = self.ui.boxed(c, parent, UiStyle { gap: Some(gap), pad: Some([2.0, 2.2].into()), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.75).outline(Tone::AccText, 0.22));
        self.ui.corners(c, bp, Tone::AccText, self.lay("corner", 1.1));
        bp
    }

    fn abs(&self, c: &mut Commands, parent: Entity, left: f32, top: f32, w: f32) -> Entity {
        self.ui.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.9), ..UiStyle::column() }.abs([Some(left), Some(top), None, None]))
    }

    fn screen(&mut self, c: &mut Commands, ambients: Option<&mut Assets<chimera_ui::AmbientMaterial>>) -> Entity {
        let ui = self.ui;
        let root = c
            .spawn((
                OnboardingRoot,
                GlobalZIndex(ONBOARDING_Z),
                UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, None]).bg(Tone::Ink),
                Node::default(),
                bevy::ui::FocusPolicy::Block,
                Interaction::None,
            ))
            .id();
        // The ground: the main menu's (the user's review, 2026-11-21: one
        // look with the menus and Settings).
        chimera_ui::shell::menu_ground(c, root, ui.theme, ambients);
        let (tag, step) = match self.o.step {
            Step::Welcome | Step::Checking => (self.w.t("welcome.tag"), Some(0)),
            Step::Hub => (self.w.t("welcome.tag"), Some(1)),
            Step::Name => (self.o.hub_name(), Some(2)),
            Step::Key => (self.o.hub_name(), Some(3)),
            Step::Have => (self.w.t("have.tag"), None),
        };
        self.bar(c, root, &tag, step);
        match self.o.step {
            Step::Welcome => self.welcome(c, root),
            Step::Checking => self.checking(c, root),
            Step::Hub => self.hub(c, root),
            Step::Name => self.name(c, root),
            Step::Key => self.key(c, root),
            Step::Have => self.have(c, root),
        }
        root
    }

    fn bar(&mut self, c: &mut Commands, root: Entity, tag: &str, step: Option<usize>) {
        let ui = self.ui;
        let m = self.lay("margin", 4.6);
        let bar = ui.boxed(c, root, UiStyle { align: Some(AlignItems::Center), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() }.abs([Some(m), Some(self.lay("bar_y", 2.6)), Some(m), None]));
        // The lockup: the game's own (the main menu's, the Esc menu's),
        // at the lobby header's size; the screen's place after a hairline.
        let brand = ui.boxed(c, bar, UiStyle { gap: Some(1.4), align: Some(AlignItems::Center), ..UiStyle::row() });
        let lockup = ui.lockup_drawn(c, brand, self.lay("lockup", 1.05));
        c.entity(lockup).insert(OnboardingLockup);
        ui.boxed(c, brand, UiStyle { w: Some(0.06), h: Some(2.0), ..UiStyle::row() }.bg(Tone::Fg4));
        ui.text(c, brand, Role::Mono, 0.6, Tone::Fg3, tag);
        if let Some(now) = step {
            let steps = ui.boxed(c, bar, UiStyle { gap: Some(0.4), ..UiStyle::row() });
            for (i, name) in self.w.list("steps").iter().enumerate() {
                let s = ui.boxed(c, steps, UiStyle { w: Some(6.2), gap: Some(0.45), ..UiStyle::column() });
                let tone = if i < now { Tone::Fg } else if i == now { Tone::Acc } else { Tone::Fg4 };
                ui.mark(c, s, tone, 5.8, 0.34);
                ui.text(c, s, Role::Mono, 0.6, if i <= now { Tone::Fg } else { Tone::Fg3 }, name);
            }
        } else {
            let back = ui.button_icon(c, bar, Kind::Tertiary, Some("back"), &self.w.t("have.back"), Some("Esc"), Size::Regular, State::NONE);
            c.entity(back).insert(ObAction::Back);
        }
        let rule = ui.boxed(c, root, UiStyle { h: Some(0.06), ..UiStyle::row() }.abs([Some(m), Some(self.lay("rule_y", 7.2)), Some(m), None]).bg(Tone::Fg5));
        ui.mark(c, rule, Tone::Acc, 6.0, 0.18);
    }

    fn keys(&mut self, c: &mut Commands, root: Entity, pairs: &[(&str, String)]) {
        let ui = self.ui;
        let row = ui.boxed(c, root, UiStyle { gap: Some(2.0), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([None, None, Some(6.4), Some(3.0)]));
        for (k, what) in pairs {
            let p = ui.boxed(c, row, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.kbd(c, p, k, 0.62, Tone::Fg2);
            ui.text(c, p, Role::Mono, 0.62, Tone::Fg3, what);
        }
    }

    fn eyebrow(&mut self, c: &mut Commands, col: Entity, text: &str) {
        let ui = self.ui;
        let e = ui.boxed(c, col, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, e, Tone::Acc, 0.7, 1.1);
        ui.text(c, e, Role::Mono, 0.66, Tone::AccText, text);
    }

    fn h1(&mut self, c: &mut Commands, col: Entity, text: &str, size: f32) {
        self.ui.text(c, col, Role::DisplayBlack, size, Tone::Fg, text);
    }

    fn lead(&mut self, c: &mut Commands, col: Entity, text: &str) {
        let b = self.ui.boxed(c, col, UiStyle { max_w: Some(38.0), ..UiStyle::column() });
        self.ui.paragraph(c, b, Role::Text, 1.02, Tone::Fg2, text);
    }

    /// A field: the kit's form field at a screen's hero size (its title,
    /// help line, the input with its eye key when masked, the status slot).
    #[allow(clippy::too_many_arguments)]
    fn field(
        &mut self,
        c: &mut Commands,
        col: Entity,
        title: &str,
        help: &str,
        key: Option<&str>,
        value: &str,
        placeholder: &str,
        caret: Option<Caret>,
        w: f32,
        st: State,
        status: Option<(Tone, &str)>,
        eye: Option<(&str, bool)>,
    ) -> Entity {
        let status = match status {
            None => FieldStatus::None,
            Some((Tone::Threat, t)) => FieldStatus::Error { text: t, fixes: &[] },
            Some((Tone::Warn, t)) => FieldStatus::Warn(t),
            Some((Tone::AccText | Tone::Acc, t)) => FieldStatus::Ok(t),
            Some((Tone::Fg3, t)) => FieldStatus::Checking(t),
            Some((_, t)) => FieldStatus::Hint(t),
        };
        let status = if st.disabled { FieldStatus::None } else { status };
        let ff = self.ui.form_field(
            c,
            col,
            &FieldSpec {
                label: title,
                help,
                key,
                value,
                placeholder,
                caret: caret.map(|k| k.cursor(value)),
                focus: st.focus,
                status,
                w,
                hero: true,
                reveal: eye,
                ..default()
            },
        );
        ff.input
    }

    #[allow(clippy::too_many_arguments)]
    fn button(&mut self, c: &mut Commands, row: Entity, kind: Kind, label: &str, key: Option<&str>, a: ObAction, st: State) -> Entity {
        // The first run's rows: every button the large size (one row, one size).
        let b = self.ui.button(c, row, kind, label, key, chimera_ui::controls::Size::Large, st);
        if !st.disabled {
            c.entity(b).insert(a);
        }
        b
    }

    fn btns(&mut self, c: &mut Commands, col: Entity, top: f32) -> Entity {
        let r = self.ui.boxed(c, col, UiStyle { gap: Some(1.1), align: Some(AlignItems::Center), margin: Some([top, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        r
    }

    /// An error: what happened, why, what to do; a threat (or warning) bar.
    fn alert(&mut self, c: &mut Commands, col: Entity, code: &str, what: &str, why: &str, todo: &str, warn: bool) {
        let ui = self.ui;
        let a = ui.boxed(c, col, UiStyle { margin: Some([1.0, 0.0, 0.0, 0.0].into()), w: Some(42.0), align: Some(AlignItems::Stretch), ..UiStyle::row() }.bg_a(Tone::Fg, 0.035));
        ui.boxed(c, a, UiStyle { w: Some(0.34), ..UiStyle::column() }.bg(if warn { Tone::Warn } else { Tone::Threat }));
        let body = ui.boxed(c, a, UiStyle { gap: Some(0.4), pad: Some([1.0, 1.6, 1.1, 1.9].into()), grow: Some(1.0), ..UiStyle::column() });
        let head = ui.boxed(c, body, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() });
        ui.text(c, head, Role::DisplayBold, 1.1, Tone::Fg, what);
        ui.text(c, head, Role::Mono, 0.58, Tone::Fg3, code);
        ui.paragraph(c, body, Role::Text, 0.9, Tone::Fg2, why);
        let d = ui.boxed(c, body, UiStyle { gap: Some(0.8), margin: Some([0.4, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.text(c, d, Role::Mono, 0.6, Tone::Fg3, "Do");
        ui.paragraph(c, d, Role::Text, 0.9, Tone::Fg, todo);
    }

    fn problem(&mut self, c: &mut Commands, col: Entity) {
        let host = self.o.host();
        let fill = [("host", host.as_str())];
        let w = self.w;
        let (code, k, warn) = match &self.o.problem {
            Some(Problem::Used) => ("Code used", "used", false),
            Some(Problem::Unknown) => ("Unknown code", "unknown", false),
            Some(Problem::Withdrawn) => ("Withdrawn", "revoked", false),
            Some(Problem::Busy) => ("Too many tries", "busy", true),
            Some(Problem::Unreachable) => ("Unreachable", "unreachable", true),
            Some(Problem::Certificate { .. }) => ("Fingerprint mismatch", "certificate", false),
            Some(Problem::Other(m)) => {
                let m = m.clone();
                self.alert(c, col, "Error", &w.t("errors.other_what"), &m, &w.t("errors.other_do"), false);
                return;
            }
            _ => return,
        };
        self.alert(
            c,
            col,
            code,
            &w.f(&format!("errors.{k}_what"), &fill),
            &w.f(&format!("errors.{k}_why"), &fill),
            &w.f(&format!("errors.{k}_do"), &fill),
            warn,
        );
        if let Some(Problem::Certificate { offered }) = &self.o.problem {
            let expected = self.o.link.as_ref().and_then(|l| l.fingerprint.clone()).unwrap_or_default();
            let rows = self.ui.kv_lit(
                c,
                col,
                &[
                    (&w.t("errors.invite_says"), short_fp(&expected), Some(Tone::Acc)),
                    (&w.t("errors.hub_showed"), offered.as_deref().map(short_fp).unwrap_or_else(|| "?".into()), Some(Tone::Threat)),
                ],
                11.0,
            );
            c.entity(rows).insert(Node { width: self.ui.units.val(36.0), margin: UiRect::top(self.ui.units.val(1.2)), ..default() });
        }
    }

    fn welcome(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.col(c, root, 0.0, self.lay("col_w", 42.0));
        self.eyebrow(c, col, &w.t("welcome.eyebrow"));
        self.h1(c, col, &w.t("welcome.title"), 3.3);
        if matches!(o.problem, None | Some(Problem::NotInvite | Problem::NeedsHub)) {
            self.lead(c, col, &w.t("welcome.lead"));
            ui.boxed(c, col, UiStyle { h: Some(0.8), ..UiStyle::column() });
        }
        let help = match o.source {
            Source::Clipboard => w.t("welcome.label_clipboard"),
            Source::Opened => w.t("welcome.label_opened"),
            Source::Typed => w.t("welcome.label"),
        };
        let bad = matches!(o.problem, Some(Problem::NotInvite | Problem::NeedsHub));
        let status_text;
        let status = match (&o.problem, &o.parsed) {
            (Some(Problem::NotInvite), _) => (Tone::Threat, w.t("welcome.not_invite")),
            (Some(Problem::NeedsHub), _) => (Tone::Warn, w.t("welcome.needs_hub")),
            _ if o.auto.is_some() => (Tone::Fg3, w.t("welcome.checking")),
            _ if o.answer.is_some() && o.valid.as_deref() == Some(o.input.trim()) => (Tone::AccText, w.f("welcome.valid", &[("hub", &o.hub_name())])),
            (_, Some(p)) => {
                let kind = match p.kind {
                    InputKind::Link => "Invite link recognised · hub, fingerprint and code",
                    InputKind::WebAddress => "Web address recognised · hub and code",
                    InputKind::Code => "Code recognised",
                };
                (Tone::AccText, kind.to_string())
            }
            _ => (Tone::Fg4, w.t("welcome.idle")),
        };
        status_text = status.1;
        let st = State { focus: true, error: bad, ..State::NONE };
        // A certificate that isn't the invite's leaves nothing to edit: the
        // field gives way to the two fingerprints (the mockups' 03d).
        if !matches!(o.problem, Some(Problem::Certificate { .. })) {
            let shown = self.reveals.shown(REVEAL_HOST);
            let (value, caret) = masked_view(field_view(&o.input, o.caret, o.parsed.as_ref()), o.parsed.as_ref(), shown);
            let has_host = o.parsed.as_ref().is_some_and(|p| p.host.is_some());
            self.field(c, col, &w.t("welcome.field"), &help, Some("Ctrl+V"), &value, &w.t("welcome.placeholder"), caret, self.lay("col_w", 42.0), st, Some((status.0, &status_text)), has_host.then_some((REVEAL_HOST, shown)));
        }
        self.problem(c, col);
        let row = self.btns(c, col, 2.2);
        // Each error's one helpful action is the hero (the mockups' 03b-d).
        let (hero, hero_key, hero_act, second) = match o.problem {
            Some(Problem::Used | Problem::Unknown | Problem::Withdrawn | Problem::Busy) => {
                ("errors.try_another", "Enter", ObAction::Clear, (Kind::Secondary, "welcome.have_account", ObAction::Have))
            }
            Some(Problem::Unreachable) => ("errors.retry", "Enter", ObAction::Retry, (Kind::Tertiary, "welcome.skip", ObAction::Skip)),
            Some(Problem::Certificate { .. }) => ("errors.back", "Esc", ObAction::Clear, (Kind::Tertiary, "errors.copy_details", ObAction::CopyDetails)),
            _ => ("welcome.continue", "Enter", ObAction::Continue, (Kind::Secondary, "welcome.have_account", ObAction::Have)),
        };
        self.button(c, row, Kind::Primary, &w.t(hero), Some(hero_key), hero_act, State::focused(true));
        self.button(c, row, second.0, &w.t(second.1), None, second.2, State::NONE);
        if let Some(said) = &o.said {
            let r = ui.boxed(c, row, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.mark(c, r, Tone::Acc, 0.6, 0.6);
            ui.text_plain(c, r, Role::Mono, 0.6, Tone::AccText, &said.to_uppercase());
        }
        // Play without an account: tertiary, but plainly a choice (decision 9).
        let skip = ui.boxed(c, root, UiStyle { gap: Some(1.2), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([Some(self.lay("col_x", 8.4) - 0.6), None, None, Some(4.4)]));
        self.button(c, skip, Kind::Tertiary, &w.t("welcome.skip"), None, ObAction::Skip, State::NONE);
        ui.text(c, skip, Role::Mono, 0.58, Tone::Fg3, &w.t("welcome.skip_note"));
        self.carries(c, root);
        if matches!(o.problem, Some(Problem::Certificate { .. })) {
            self.keys(c, root, &[("Esc", w.t(hero))]);
        } else {
            let enter = if o.problem.is_some() && !bad { w.t(hero) } else { w.t("welcome.continue") };
            self.keys(c, root, &[("Enter", enter), ("Esc", "Clear".into())]);
        }
    }

    /// The pane: what an invite carries, each part lit as it is found.
    fn carries(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let pane = self.pane(c, root, 0.0, 0.0, self.lay("pane_w", 33.6));
        let bp = self.blueprint(c, pane, 0.0);
        let head = ui.boxed(c, bp, UiStyle { justify: Some(JustifyContent::SpaceBetween), margin: Some([0.0, 0.0, 1.4, 0.0].into()), ..UiStyle::row() });
        ui.text(c, head, Role::Mono, 0.62, Tone::AccText, &w.t("welcome.pane_title"));
        let p = o.parsed.as_ref();
        let kind = match p.map(|p| p.kind) {
            Some(InputKind::Link) => "Link · complete",
            Some(InputKind::WebAddress) => "Web address",
            Some(InputKind::Code) => "Bare code",
            None if o.problem == Some(Problem::NotInvite) => "Not recognised",
            None => "Waiting",
        };
        ui.text(c, head, Role::Mono, 0.52, if p.is_some() { Tone::AccText } else { Tone::Fg3 }, kind);
        let last = o.last_hub.as_ref().map(|l| l.host.clone());
        for i in 0..3 {
            let (state, value): (Option<(Tone, &str)>, String) = match (p, i) {
                (None, _) => (None, w.at("welcome.parts_idle", i)),
                (Some(p), 0) => match (&p.host, &last) {
                    (Some(h), _) => (Some((Tone::Acc, "Found")), secret::shown(h, self.reveals.shown(REVEAL_HOST))),
                    (None, Some(h)) => (Some((Tone::Warn, "Last used")), secret::shown(h, self.reveals.shown(REVEAL_HOST))),
                    (None, None) => (Some((Tone::Warn, "Ask")), w.t("welcome.needs_hub")),
                },
                (Some(p), 1) => match &p.fingerprint {
                    Some(f) => (Some((Tone::Acc, "Found")), format!("SHA-256 {}", short_fp(f))),
                    None => (Some((Tone::Warn, "From the hub")), w.t("welcome.from_hub")),
                },
                (Some(p), _) => match &p.code {
                    Some(code) => (Some((Tone::Acc, "Found")), code.to_uppercase()),
                    None => (Some((Tone::Warn, "Missing")), w.t("welcome.missing")),
                },
            };
            let part = ui.boxed(c, bp, UiStyle { gap: Some(0.4), pad: Some([1.0, 0.0].into()), ..UiStyle::column() });
            ui.hairline(c, part, Tone::Fg, 0.07);
            let top = ui.boxed(c, part, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.text(c, top, Role::Mono, 0.62, Tone::Fg3, &format!("0{}", i + 1));
            ui.text(c, top, Role::DisplayBold, 0.82, if state.is_some() { Tone::Fg } else { Tone::Fg3 }, &w.at("welcome.parts", i));
            ui.boxed(c, top, UiStyle::row().grow(1.0));
            if let Some((tone, word)) = state {
                ui.mark(c, top, tone, 0.6, 0.6);
                ui.text(c, top, Role::Mono, 0.56, if tone == Tone::Acc { Tone::AccText } else { Tone::Warn }, word);
            }
            let v = ui.boxed(c, part, UiStyle { margin: Some([0.0, 0.0, 0.0, 3.2].into()), ..UiStyle::row() });
            ui.text_plain(c, v, Role::Mono, 0.68, if state.is_some() { Tone::Fg2 } else { Tone::Fg3 }, &value);
        }
        // The link's shape, each part numbered under it: the player sees
        // where in the link each one lives.
        let ex = w.list("welcome.diagram");
        let ex = |i: usize| ex.get(i).cloned().unwrap_or_default();
        let parts = match p {
            Some(p) => [
                p.host.as_deref().map(|h| secret::shown(h, self.reveals.shown(REVEAL_HOST))).unwrap_or_else(|| ex(0)),
                p.fingerprint.as_deref().map(short_link_fp).unwrap_or_else(|| ex(1)),
                p.code.clone().map(|c| c.to_uppercase()).unwrap_or_else(|| ex(2)),
            ],
            None => [ex(0), ex(1), ex(2)],
        };
        let d = ui.boxed(c, bp, UiStyle { align: Some(AlignItems::FlexStart), margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        let seg = |c: &mut Commands, text: &str, n: Option<usize>| {
            let col = ui.boxed(c, d, UiStyle { gap: Some(0.25), ..UiStyle::column() });
            ui.text_plain(c, col, Role::Mono, 0.56, if n.is_some() { Tone::Fg2 } else { Tone::Fg3 }, text);
            if let Some(n) = n {
                ui.boxed(c, col, UiStyle { h: Some(0.06), ..UiStyle::row() }.bg(Tone::AccText));
                ui.text(c, col, Role::Mono, 0.5, Tone::AccText, &format!("0{n}"));
            }
        };
        seg(c, "chimera-hub://", None);
        seg(c, &parts[0], Some(1));
        seg(c, "/", None);
        seg(c, &parts[1], Some(2));
        seg(c, "?code=", None);
        seg(c, &parts[2], Some(3));
        let note = ui.boxed(c, pane, UiStyle { margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("welcome.pane_note"));
    }

    fn checking(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.col(c, root, 0.0, self.lay("col_w", 42.0));
        self.eyebrow(c, col, &w.t("welcome.eyebrow"));
        self.h1(c, col, &w.t("checking.title"), 3.3);
        ui.boxed(c, col, UiStyle { h: Some(1.6), ..UiStyle::column() });
        let parsed = InviteInput::parse(&o.input);
        let shown = self.reveals.shown(REVEAL_HOST);
        let (value, _) = masked_view(field_view(&o.input, o.caret, parsed.as_ref()), parsed.as_ref(), shown);
        self.field(c, col, &w.t("welcome.field"), &w.t("welcome.label"), None, &value, "", None, self.lay("col_w", 42.0), State { disabled: true, ..State::NONE }, None, Some((REVEAL_HOST, shown)));
        let bar = ui.boxed(c, col, UiStyle { margin: Some([2.2, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.progress(c, bar, chimera_ui::controls::Progress::Connecting, &w.t("checking.connecting"), 30.0, 1.7, 0.62, true);
        let host = secret::shown(&o.host(), shown);
        // Each check with its light: under way (accent) or waiting (unlit).
        let rows = [
            (w.at("checking.rows", 0), host, Some(Tone::Acc)),
            (w.at("checking.rows", 1), w.at("checking.states", 0), Some(Tone::Acc)),
            (w.at("checking.rows", 2), w.at("checking.states", 1), None),
        ];
        let rows: Vec<(&str, String, Option<Tone>)> = rows.iter().map(|(k, v, l)| (k.as_str(), v.clone(), *l)).collect();
        let kv = ui.kv_lit(c, col, &rows, 11.0);
        c.entity(kv).insert(Node { width: ui.units.val(30.0), margin: UiRect::top(ui.units.val(1.8)), ..default() });
        let row = self.btns(c, col, 2.4);
        self.button(c, row, Kind::Secondary, &w.t("checking.cancel"), Some("Esc"), ObAction::Back, State::NONE);
    }

    fn hub(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let a = o.answer.clone().unwrap_or_default();
        let col = self.col(c, root, 1.8, 44.0);
        self.eyebrow(c, col, &w.t("hub.eyebrow"));
        let t = ui.text(c, col, Role::Mono, 0.8, Tone::Fg2, &w.t("hub.joining"));
        c.entity(t).insert(Node { margin: UiRect::top(ui.units.val(2.4)), ..default() });
        self.h1(c, col, &a.hub.name, 4.6);
        let pinned = o.link.as_ref().is_some_and(|l| l.fingerprint.is_some());
        let shown = self.reveals.shown(REVEAL_HOST);
        let pairs = [
            (w.at("hub.rows", 0), secret::shown(&o.host(), shown)),
            (w.at("hub.rows", 1), if pinned { w.t("hub.matches") } else { w.t("hub.pinned_none") }),
            (w.at("hub.rows", 2), a.hub.players.to_string()),
        ];
        let pairs: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let pairs: Vec<(&str, String, Option<Tone>)> = pairs.iter().enumerate().map(|(i, (k, v))| (*k, v.clone(), Some(if i == 1 && pinned { Tone::Acc } else { Tone::Fg2 }))).collect();
        let kv = ui.kv_lit(c, col, &pairs, 11.0);
        c.entity(kv).insert(Node { width: ui.units.val(40.0), margin: UiRect::top(ui.units.val(2.2)), ..default() });
        // The address's eye key, on its line's right end.
        let eye = ui.boxed(c, kv, UiStyle { w: Some(2.0), h: Some(2.0), ..UiStyle::row() }.abs([None, Some(0.1), Some(0.0), None]));
        ui.reveal_key(c, eye, REVEAL_HOST, shown, 2.0);
        let row = self.btns(c, col, 2.4);
        self.button(c, row, Kind::Primary, &w.f("hub.join", &[("hub", &a.hub.name)]), Some("Enter"), ObAction::Continue, State::focused(true));
        self.button(c, row, Kind::Secondary, &w.t("hub.not_this"), None, ObAction::Back, State::NONE);
        // The MOTD plate.
        let pane = self.pane(c, root, 2.0, 5.8, 31.6);
        let plate = ui.boxed(c, pane, UiStyle { gap: Some(1.2), pad: Some([2.2, 2.4].into()), ..UiStyle::column() }.bg(Tone::Plate).outline(Tone::Fg5, 1.0));
        ui.boxed(c, plate, UiStyle { h: Some(0.3), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), Some(0.0), None]).bg(Tone::Fg));
        let h = ui.boxed(c, plate, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, h, Tone::Acc, 0.9, 1.4);
        ui.text(c, h, Role::Mono, 0.62, Tone::Fg, &w.t("hub.motd"));
        let motd = if a.hub.motd.trim().is_empty() { w.t("hub.no_motd") } else { a.hub.motd.clone() };
        ui.paragraph(c, plate, Role::Text, 1.12, Tone::Fg, &motd);
        // The hub's emblem (its initials on the hero slant) and the note.
        let foot = ui.boxed(c, pane, UiStyle { gap: Some(1.6), align: Some(AlignItems::Center), margin: Some([2.0, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        let em = ui.boxed(c, foot, UiStyle { w: Some(5.6), h: Some(5.6), align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() });
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, ..default() },
            chimera_ui::style::UiSlant::new(Tone::Hero, 1.0),
            Pickable::IGNORE,
            ChildOf(em),
        ));
        ui.text(c, em, Role::DisplayBlack, 1.4, Tone::OnHero, &initials(&a.hub.name));
        let note = ui.boxed(c, foot, UiStyle { max_w: Some(20.0), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("hub.note"));
        self.keys(c, root, &[("Enter", "Join".into()), ("Esc", "Back".into())]);
    }

    fn avatar(&mut self, c: &mut Commands, parent: Entity, byte: u8, size: f32) -> Entity {
        let look = AvatarLook::from_byte(byte).unwrap_or(AvatarLook::new(0, 0));
        match (self.images.as_deref_mut(), self.icons.as_deref_mut()) {
            (Some(images), Some(icons)) => chimera_ui::avatar::spawn(self.ui, c, parent, images, icons, Face::Look(look), AvatarMarks::default(), size),
            _ => {
                let e = self.ui.boxed(c, parent, UiStyle { w: Some(size), h: Some(size), ..UiStyle::row() });
                c.entity(e).insert(BackgroundColor(look.ground()));
                e
            }
        }
    }

    fn name(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let hub = o.hub_name();
        // What you choose, at the left: the two fields, then the avatar
        // filling the room beside them; at the right only what follows
        // from it (the user's review, 2026-11-21).
        let col = self.col(c, root, -1.0, self.lay("name_w", 56.0));
        self.eyebrow(c, col, &w.t("name.eyebrow"));
        self.h1(c, col, &w.t("name.title"), 3.0);
        let two = ui.boxed(c, col, UiStyle { gap: Some(2.6), margin: Some([1.2, 0.0, 0.0, 0.0].into()), align: Some(AlignItems::FlexStart), ..UiStyle::row() });
        let fw = self.lay("name_field_w", 25.0);
        let fields = ui.boxed(c, two, UiStyle { w: Some(fw), gap: Some(1.6), shrink: Some(0.0), ..UiStyle::column() });
        let folded = o.username.trim().to_ascii_lowercase();
        let rules = UsernameRules::of(&folded);
        let taken = o.name.as_ref().is_some_and(|n| n.username == folded && !n.available);
        let checking = rules.all() && !taken && !o.name_ok();
        let (tone, status) = if folded.is_empty() {
            (Tone::Fg4, w.t("name.empty"))
        } else if !rules.all() {
            (Tone::Threat, w.t("name.fix"))
        } else if taken {
            (Tone::Threat, w.f("name.taken", &[("hub", &hub), ("name", &folded)]))
        } else if o.name_ok() {
            (Tone::AccText, w.f("name.available", &[("hub", &hub)]))
        } else {
            (Tone::Fg4, w.f("name.checking", &[("hub", &hub)]))
        };
        let count = format!("{} / {}", folded.chars().count(), codes::NAME_MAX);
        let free = o.name.as_ref().map(|n| n.suggestions.clone()).unwrap_or_default();
        let taken_line = format!("{status} · {}", w.t("name.free"));
        let status = if folded.is_empty() {
            FieldStatus::Hint(&status)
        } else if !rules.all() {
            FieldStatus::Error { text: &status, fixes: &[] }
        } else if taken {
            FieldStatus::Error { text: &taken_line, fixes: &free }
        } else if o.name_ok() {
            FieldStatus::Ok(&status)
        } else {
            FieldStatus::Checking(&status)
        };
        let _ = tone;
        let (ulabel, uhelp, req) = (w.t("name.username"), w.f("name.username_help", &[("hub", &hub)]), w.t("name.required"));
        let uf = ui.form_field(
            c,
            fields,
            &FieldSpec {
                label: &ulabel,
                tag: Some(&req),
                help: &uhelp,
                value: &o.username,
                caret: (o.field == NameField::Username).then(|| o.ucaret.cursor(&o.username)),
                focus: o.field == NameField::Username,
                status,
                w: fw,
                ..default()
            },
        );
        c.entity(uf.input).insert(ObAction::Focus(NameField::Username));
        for (e, s) in uf.fixes.iter().zip(&free) {
            c.entity(*e).insert(ObAction::Suggest(s.clone()));
        }
        // The rules, lit as typed.
        let grid = ui.boxed(c, uf.root, UiStyle { gap: Some(0.4), margin: Some([0.2, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        let met = [rules.length, rules.characters, rules.starts_with_letter, rules.all() && o.name_ok()];
        for i in 0..4 {
            let cell = ui.boxed(c, grid, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
            let lit = if folded.is_empty() || (i == 3 && (checking || !rules.all())) {
                Tone::Fg4
            } else if met[i] {
                Tone::Acc
            } else {
                Tone::Threat
            };
            ui.mark(c, cell, lit, 0.45, 0.45);
            ui.text_plain(c, cell, Role::Mono, 0.52, if lit == Tone::Fg4 { Tone::Fg3 } else if lit == Tone::Acc { Tone::AccText } else { Tone::Threat }, &w.at("name.rules", i));
        }
        let (dlabel, dhelp, opt) = (w.t("name.display"), w.t("name.display_help"), w.t("name.optional"));
        let dcount = format!("{opt} · {count}");
        let dproblem = display_problem(&o.display);
        let df = ui.form_field(
            c,
            fields,
            &FieldSpec {
                label: &dlabel,
                tag: Some(&dcount),
                help: &dhelp,
                value: &o.display,
                placeholder: if folded.is_empty() { "" } else { &folded },
                caret: (o.field == NameField::Display).then(|| o.dcaret.cursor(&o.display)),
                focus: o.field == NameField::Display,
                status: dproblem.as_deref().map_or(FieldStatus::None, |e| FieldStatus::Error { text: e, fixes: &[] }),
                w: fw,
                ..default()
            },
        );
        c.entity(df.input).insert(ObAction::Focus(NameField::Display));
        // The avatar: the 32 looks (decision 10), as large as the room
        // beside the fields allows.
        let aw = self.lay("name_w", 56.0) - fw - 2.6;
        let av = ui.boxed(c, two, UiStyle { w: Some(aw), gap: Some(0.5), shrink: Some(0.0), ..UiStyle::column() });
        let on = o.field == NameField::Avatar;
        ui.form_section(c, av, &w.t("name.avatar"), Some(&format!("{} / 32", o.avatar as u32 + 1)), Some(&w.t("name.avatar_help")));
        let (images, icons) = (self.images.as_deref_mut(), self.icons.as_deref_mut());
        for (b, t) in ui.avatar_grid(c, av, images, icons, o.avatar, on, aw).into_iter().enumerate() {
            c.entity(t).insert(ObAction::Avatar(b as u8));
        }
        let note = ui.boxed(c, col, UiStyle { margin: Some([1.4, 0.0, 0.0, 0.0].into()), max_w: Some(50.0), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("name.note"));
        self.problem(c, col);
        let row = self.btns(c, col, 1.2);
        let ready = o.name_ok() && display_problem(&o.display).is_none() && o.pending.is_none();
        self.button(c, row, Kind::Primary, &w.t("name.continue"), Some("Enter"), ObAction::Continue, State { focus: ready, disabled: !ready, ..State::NONE });
        self.button(c, row, Kind::Tertiary, &w.t("name.back"), None, ObAction::Back, State::NONE);
        // The preview: how others see you (nothing to change here).
        let shown = if o.display.trim().is_empty() { if folded.is_empty() { "...".to_string() } else { folded.clone() } } else { o.display.trim().to_string() };
        let pane = self.pane(c, root, self.lay("name_pane_dx", 9.0), 1.8, self.lay("name_pane_w", 27.0));
        let bp = self.blueprint(c, pane, 0.8);
        let head = ui.boxed(c, bp, UiStyle { justify: Some(JustifyContent::SpaceBetween), margin: Some([0.0, 0.0, 0.6, 0.0].into()), ..UiStyle::row() });
        ui.text(c, head, Role::Mono, 0.62, Tone::AccText, &w.t("name.preview"));
        ui.text(c, head, Role::Mono, 0.52, Tone::Fg3, "Preview");
        ui.text(c, bp, Role::Mono, 0.56, Tone::Fg3, &w.t("name.lobby"));
        let lobby = ui.boxed(c, bp, UiStyle { gap: Some(1.2), align: Some(AlignItems::Center), h: Some(3.4), pad: Some([0.0, 1.0].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.07).outline(Tone::Fg2, 1.0));
        ui.text(c, lobby, Role::Mono, 0.6, Tone::Fg3, "01");
        self.avatar(c, lobby, o.avatar, 2.4);
        ui.text_plain(c, lobby, Role::DisplayBold, 1.0, Tone::Fg, &shown);
        ui.boxed(c, lobby, UiStyle::row().grow(1.0));
        let you = ui.boxed(c, lobby, UiStyle { pad: Some([0.2, 0.4].into()), ..UiStyle::row() }.bg(Tone::Fg));
        ui.text(c, you, Role::Mono, 0.52, Tone::Ink, &w.t("name.you"));
        // The kill feed: your kill, outlined and barred in accent.
        ui.text(c, bp, Role::Mono, 0.56, Tone::Fg3, &w.t("name.feed"));
        let feed = ui.boxed(c, bp, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), h: Some(2.2), pad: Some([0.0, 1.0, 0.0, 1.3].into()), ..UiStyle::row() }.bg_a(Tone::Ink, 0.8).outline(Tone::Acc, 1.0));
        ui.boxed(c, feed, UiStyle { w: Some(0.3), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), None, Some(0.0)]).bg(Tone::Acc));
        ui.text_plain(c, feed, Role::DisplayBold, 0.86, Tone::AccText, &shown);
        ui.kit_icon(c, feed, "kill", 0.9, 1.0, Tone::Fg);
        ui.text_plain(c, feed, Role::Text, 0.86, Tone::Fg2, &w.t("name.feed_victim"));
        ui.text(c, bp, Role::Mono, 0.56, Tone::Fg3, &w.t("name.profile"));
        ui.kv(c, bp, &[("Username", format!("@{}", if folded.is_empty() { "..." } else { &folded })), ("Hub", hub.clone())], 7.0);
        self.keys(c, root, &[("Tab", "Next field".into()), ("Enter", "Continue".into())]);
    }

    fn key(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let user = o.signed_in.as_ref().map(|f| f.username.clone()).unwrap_or_default();
        let hub = o.hub_name();
        let col = self.col(c, root, 0.0, 46.0);
        self.eyebrow(c, col, &w.t("key.eyebrow"));
        self.h1(c, col, &w.t("key.title"), 2.7);
        self.lead(c, col, &w.t("key.lead"));
        let label = w.t("key.field");
        let help = w.f("key.label", &[("user", &user), ("hub", &hub)]);
        let tag = w.t("key.tag");
        let key = o.recovery.clone().unwrap_or_default();
        // Hidden until its eye key is pressed (streamers); its dashes show
        // (a fixed format), nothing else. Copy and Save never need it shown.
        let copied = o.said.as_deref() == Some(w.t("key.copied").as_str());
        ui.boxed(c, col, UiStyle { h: Some(1.6), ..UiStyle::column() });
        let sf = ui.secret_field(
            c,
            col,
            &SecretSpec { label: &label, tag: Some(&tag), help: &help, key: REVEAL_KEY, value: &key, revealed: self.reveals.shown(REVEAL_KEY), fixed: true, copied, w: 46.0 },
        );
        c.entity(sf.copy).insert(ObAction::Copy);
        let row = self.btns(c, col, 0.4);
        self.button(c, row, Kind::Secondary, &w.t("key.save"), Some("Ctrl+S"), ObAction::Save, State::NONE);
        if let Some(s) = o.said.as_ref().filter(|s| o.saved_path.is_none() && !copied && s.as_str() != w.t("key.saved")) {
            let r = ui.boxed(c, row, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.mark(c, r, Tone::Acc, 0.5, 0.5);
            ui.text_plain(c, r, Role::Mono, 0.56, Tone::AccText, &s.to_uppercase());
        }
        // Where the file went: its whole path on its own line, Show in folder.
        if let Some(path) = &o.saved_path {
            ui.boxed(c, col, UiStyle { h: Some(0.4), ..UiStyle::column() });
            let (_, show) = ui.path_row(c, col, &w.t("key.saved_to"), path, &w.t("key.show_folder"), 46.0);
            c.entity(show).insert(ObAction::ShowFolder);
        }
        let row = self.btns(c, col, 1.8);
        let check = ui.check(c, row, o.saved, &w.t("key.saved_it"), 1.5);
        c.entity(check).insert(ObAction::Saved);
        ui.boxed(c, row, UiStyle { w: Some(2.0), ..UiStyle::row() });
        self.button(c, row, Kind::Primary, &w.t("key.continue"), Some("Enter"), ObAction::Continue, State { focus: o.saved, disabled: !o.saved, ..State::NONE });
        // Getting back in.
        let pane = self.pane(c, root, 2.0, 3.0, 31.6);
        let bp = self.blueprint(c, pane, 0.0);
        let head = ui.boxed(c, bp, UiStyle { justify: Some(JustifyContent::SpaceBetween), margin: Some([0.0, 0.0, 1.2, 0.0].into()), ..UiStyle::row() });
        ui.text(c, head, Role::Mono, 0.62, Tone::AccText, &w.t("key.pane_title"));
        ui.text(c, head, Role::Mono, 0.52, Tone::Fg3, "3 ways");
        for i in 0..3 {
            let part = ui.boxed(c, bp, UiStyle { gap: Some(0.4), pad: Some([1.0, 0.0].into()), ..UiStyle::column() });
            ui.hairline(c, part, Tone::Fg, 0.07);
            let top = ui.boxed(c, part, UiStyle { gap: Some(1.0), ..UiStyle::row() });
            ui.text(c, top, Role::Mono, 0.62, Tone::Fg3, &format!("0{}", i + 1));
            ui.text(c, top, Role::DisplayBold, 0.82, Tone::Fg, &w.at("key.ways", i));
            let how = ui.boxed(c, part, UiStyle { margin: Some([0.0, 0.0, 0.0, 3.2].into()), ..UiStyle::column() });
            ui.paragraph(c, how, Role::Text, 0.8, Tone::Fg2, &w.at("key.ways_how", i));
        }
        let note = ui.boxed(c, pane, UiStyle { margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("key.pane_note"));
        self.keys(c, root, &[("Ctrl+C", "Copy key".into()), ("Ctrl+S", "Save as file".into()), ("Space", w.t("key.saved_it")), ("Enter", "Continue".into())]);
    }

    fn have(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.col(c, root, -2.0, 46.0);
        self.h1(c, col, &w.t("have.title"), 3.3);
        let tabs_row = ui.boxed(c, col, UiStyle { margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        let names = w.list("have.tabs");
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let sel = if o.tab == HaveTab::Link { 0 } else { 1 };
        for (i, t) in ui.tabs_ex(c, tabs_row, &names, sel, true, Some(("Q", "E"))).into_iter().enumerate() {
            c.entity(t).insert(ObAction::Tab(if i == 0 { HaveTab::Link } else { HaveTab::Recover }));
        }
        let link = o.tab == HaveTab::Link;
        self.lead(c, col, &w.t(if link { "have.link_lead" } else { "have.recover_lead" }));
        ui.boxed(c, col, UiStyle { h: Some(1.4), ..UiStyle::column() });
        if o.last_hub.is_none() {
            let st = State::focused(o.have_on_hub);
            let parsed = InviteInput::parse(&o.have_hub).filter(|p| p.host.is_some());
            let shown = self.reveals.shown(REVEAL_HOST);
            let (value, caret) = masked_view((o.have_hub.clone(), o.have_on_hub.then(|| Caret::end_of(&o.have_hub))), parsed.as_ref(), shown);
            let hf = self.field(c, col, &w.t("have.hub_label"), &w.t("have.hub_help"), Some("Ctrl+V"), &value, "chimera-hub://...", caret, self.lay("col_w", 42.0), st, None, parsed.is_some().then_some((REVEAL_HOST, shown)));
            c.entity(hf).insert(ObAction::HaveField(true));
            ui.boxed(c, col, UiStyle { h: Some(1.0), ..UiStyle::column() });
        }
        let st = State { focus: !o.have_on_hub, error: o.problem.is_some(), ..State::NONE };
        let cells: Vec<char> = o.have_code.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
        if link && cells.len() <= LINK_CELLS {
            // A link code: one cell a character (the mockups' 08a), like
            // pairing a TV app; the caret in the next empty one.
            self.code_cells(c, col, &cells, st);
        } else {
            let (label, ph, status) = if link {
                (w.t("have.link_label"), "H4K-92Q", Some((Tone::Fg4, w.t("have.link_status"))))
            } else {
                (w.t("have.recover_label"), "XXXX-XXXX-XXXX-XXXX", None)
            };
            let status_ref = status.as_ref().map(|(t, s)| (*t, s.as_str()));
            let cf = self.field(c, col, &label, "", None, &o.have_code, ph, (!o.have_on_hub).then(|| Caret::end_of(&o.have_code)), self.lay("col_w", 42.0), st, status_ref, None);
            c.entity(cf).insert(ObAction::HaveField(false));
        }
        if let Some(Problem::NeedsHub) = o.problem {
            let r = ui.boxed(c, col, UiStyle { gap: Some(0.8), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
            ui.mark(c, r, Tone::Warn, 0.6, 0.6);
            ui.text(c, r, Role::Mono, 0.6, Tone::Warn, &w.t("welcome.needs_hub"));
        } else if o.problem.is_some() {
            self.problem(c, col);
        }
        let row = self.btns(c, col, 2.2);
        let label = w.t(if link { "have.link_button" } else { "have.recover_button" });
        let ready = !o.have_code.trim().is_empty() && o.pending.is_none();
        self.button(c, row, Kind::Primary, &label, Some("Enter"), ObAction::Continue, State { focus: ready, disabled: !ready, ..State::NONE });
        let note = ui.boxed(c, col, UiStyle { margin: Some([2.2, 0.0, 0.0, 0.0].into()), max_w: Some(40.0), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t(if link { "have.lost" } else { "have.recover_note" }));
        if link {
            self.other_pc(c, root);
        }
        self.keys(c, root, &[("Q / E", "Tab".into()), ("Enter", "Sign in".into()), ("Esc", "Back".into())]);
        let _ = SENTENCE;
    }

    /// The link code's cells: its label, six cells with a dash after the
    /// third, the status line.
    fn code_cells(&mut self, c: &mut Commands, col: Entity, chars: &[char], st: State) {
        let (ui, w) = (self.ui, self.w);
        let f = ui.boxed(c, col, UiStyle { gap: Some(0.8), ..UiStyle::column() });
        ui.text(c, f, Role::Mono, 0.56, Tone::Fg3, &w.t("have.link_label"));
        let row = ui.item(c, f, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() }, State::NONE, false);
        c.entity(row).insert(ObAction::HaveField(false));
        for i in 0..LINK_CELLS {
            if i == LINK_CELLS / 2 {
                ui.boxed(c, row, UiStyle { w: Some(1.2), h: Some(0.2), ..UiStyle::row() }.bg(Tone::Fg3));
            }
            let cur = st.focus && i == chars.len();
            let cell = ui.boxed(
                c,
                row,
                UiStyle { w: Some(3.4), h: Some(4.3), align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), ..UiStyle::row() }.bg_a(Tone::Fg, if cur { 0.07 } else { 0.04 }),
            );
            c.entity(cell).insert(Pickable::IGNORE);
            match chars.get(i) {
                Some(ch) => {
                    ui.text_plain(c, cell, Role::Mono, 2.0, Tone::Fg, &ch.to_string());
                }
                None if cur => {
                    ui.boxed(c, cell, UiStyle { w: Some(0.08), h: Some(1.9), ..UiStyle::row() }.bg(Tone::Fg));
                }
                None => {}
            }
            let line = if st.error { Tone::Threat } else if cur { Tone::Acc } else if i < chars.len() { Tone::Fg2 } else { Tone::Fg4 };
            ui.boxed(c, cell, UiStyle { h: Some(0.2), ..UiStyle::row() }.abs([Some(0.0), None, Some(0.0), Some(0.0)]).bg(line));
        }
        let sr = ui.boxed(c, f, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), margin: Some([0.4, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        ui.mark(c, sr, Tone::Fg4, 0.6, 0.6);
        ui.text_plain(c, sr, Role::Mono, 0.62, Tone::Fg3, &w.t("have.link_status").to_uppercase());
    }

    /// Where the code comes from: the other PC's Settings, drawn small at
    /// the bottom right (the mockups' inset).
    fn other_pc(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w) = (self.ui, self.w);
        let m = self.lay("margin", 4.6);
        let plate = ui.boxed(c, root, UiStyle { w: Some(30.0), ..UiStyle::column() }.abs([None, None, Some(m), Some(m + 2.4)]).bg(Tone::Plate).outline(Tone::Fg5, 1.0));
        ui.boxed(c, plate, UiStyle { h: Some(0.3), ..UiStyle::row() }.bg(Tone::Fg3));
        let body = ui.boxed(c, plate, UiStyle { gap: Some(0.7), pad: Some([1.4, 1.8, 1.6, 1.8].into()), ..UiStyle::column() });
        ui.text(c, body, Role::Mono, 0.56, Tone::Fg3, &w.t("have.other_where"));
        ui.text(c, body, Role::DisplayBold, 0.9, Tone::Fg, &w.t("have.other_title"));
        ui.text_plain(c, body, Role::Mono, 2.4, Tone::Fg, &w.t("have.other_example"));
        let ticks = ui.boxed(c, body, UiStyle { gap: Some(0.18), ..UiStyle::row() });
        for i in 0..16 {
            ui.mark(c, ticks, if i < 12 { Tone::Fg } else { Tone::Fg4 }, 1.4, 0.6);
        }
        ui.text(c, body, Role::Mono, 0.56, Tone::Fg3, &w.t("have.other_note"));
    }
}

/// A link code's cells.
const LINK_CELLS: usize = 6;

/// A hub's initials for its emblem: the first letters of its first two
/// words ("Friday Night Hub": FN).
pub fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().find(|c| c.is_alphanumeric())).take(2).flat_map(char::to_uppercase).collect()
}

/// A fingerprint as the invite field shows it inside a link: its first
/// and last four, lower case (`3f9a..6e6f`).
fn short_link_fp(f: &str) -> String {
    let f: String = f.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if f.len() <= 8 {
        return f;
    }
    format!("{}..{}", &f[..4], &f[f.len() - 4..])
}

/// Characters the invite field shows (42 cq at the hero field's typed
/// size, `[layout.field] hero_text`).
const FIELD_CHARS: usize = 64;

/// `view` (`field_view`'s) with the hub's address masked unless shown
/// (`chimera_ui::secret`); the caret hidden while masked (typing goes on).
fn masked_view(view: (String, Option<Caret>), parsed: Option<&InviteInput>, shown: bool) -> (String, Option<Caret>) {
    match parsed.and_then(|p| p.host.as_deref()).filter(|h| !shown && !h.is_empty() && view.0.contains(*h)) {
        Some(h) => (view.0.replace(h, &secret::mask(h)), None),
        None => view,
    }
}

/// What the invite field shows: a link read at Enter with its fingerprint
/// shortened (no caret: Esc clears, typing starts over), else the text's
/// last `FIELD_CHARS` with the caret moved along.
fn field_view(text: &str, caret: Caret, parsed: Option<&InviteInput>) -> (String, Option<Caret>) {
    if let Some(p) = parsed.filter(|p| p.kind == InputKind::Link && text.chars().count() > FIELD_CHARS) {
        let mut out = format!("chimera-hub://{}/", p.host.clone().unwrap_or_default());
        if let Some(f) = &p.fingerprint {
            out.push_str(&short_link_fp(f));
        }
        if let Some(code) = &p.code {
            out.push_str(&format!("?code={code}"));
        }
        return (out, None);
    }
    let n = text.chars().count();
    if n <= FIELD_CHARS {
        return (text.to_string(), Some(caret));
    }
    let skip = n - FIELD_CHARS + 2;
    let shown: String = format!("..{}", text.chars().skip(skip).collect::<String>());
    let at = caret.at.saturating_sub(skip) + 2;
    (shown, Some(Caret { at, anchor: at }))
}

/// A fingerprint as the screens show it: `3F9A 0B71 … 5D4E C21E`.
pub fn short_fp(f: &str) -> String {
    let f: String = f.to_uppercase().chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if f.len() <= 16 {
        return f.as_bytes().chunks(4).map(|g| String::from_utf8_lossy(g).into_owned()).collect::<Vec<_>>().join(" ");
    }
    format!("{} {} .. {} {}", &f[..4], &f[4..8], &f[f.len() - 8..f.len() - 4], &f[f.len() - 4..])
}

// ---------------------------------------------------------------------------
// The first-launch intro (module doc, "Intro").

/// The intro's length at motion scale 1, seconds; its last `INTRO_OUT`
/// fade it into Welcome.
pub const INTRO_SECS: f32 = 2.6;
pub const INTRO_OUT: f32 = 0.3;
/// How long its last frame stands at motion scale 0, seconds.
pub const INTRO_STILL: f32 = 0.6;
/// The lockup's size in the intro (its wordmark's, cq).
const INTRO_LK: f32 = 3.0;

/// The intro's length at motion scale `scale`, seconds.
pub fn intro_len(scale: f32) -> f32 {
    if scale <= 0.0 { INTRO_STILL } else { INTRO_SECS * scale }
}

/// When Welcome's entry starts, seconds after the intro's: under the
/// intro's fade out (at motion scale 0, as it ends).
fn welcome_at(scale: f32) -> f64 {
    if scale <= 0.0 { INTRO_STILL as f64 } else { ((INTRO_SECS - INTRO_OUT) * scale) as f64 }
}

/// Ends the intro (on its own, or `skipped` by a key or click: Welcome
/// then shows settled at once) and writes its mark, so it plays once.
pub fn end_intro(o: &mut Onboarding, files: &OnboardingFiles, now: f64, skipped: bool) {
    if o.intro.take().is_none() {
        return;
    }
    if skipped {
        o.since = now - 10.0;
    } else if o.since > now {
        o.since = now;
    }
    if let Some(mark) = &files.intro_mark {
        if let Some(dir) = mark.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(mark, "") {
            warn!("onboarding: the intro's mark {}: {e}", mark.display());
        }
    }
}

/// The intro's root (over the first run's screens).
#[derive(Component)]
pub struct IntroRoot;

/// A node the intro animates (`hold_intro` keeps it still when held).
#[derive(Component)]
pub struct IntroPart;

/// The intro's timecode (`intro_clock`).
#[derive(Component)]
struct IntroClock;

/// The intro's end, its skip by a click, and its sound cues.
#[allow(clippy::too_many_arguments)]
fn intro_tick(
    mut o: ResMut<Onboarding>,
    files: Res<OnboardingFiles>,
    clock: Res<UiClock>,
    motion: Option<Res<Motion>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    pressed: Query<&Interaction, (With<IntroRoot>, Changed<Interaction>)>,
    (cues, warmth): (Option<ResMut<Messages<chimera_ui::UiCue>>>, Option<Res<chimera_ui::Warmth>>),
    mut sent: Local<(f64, u8)>,
) {
    let Some(mut intro) = o.intro else { return };
    let now = clock.now;
    if !o.active {
        o.intro = None;
        return;
    }
    let clicked = mouse.is_some_and(|m| m.get_just_pressed().next().is_some()) || pressed.iter().any(|i| *i == Interaction::Pressed);
    if clicked {
        end_intro(&mut o, &files, now, true);
        return;
    }
    let scale = motion.map_or(1.0, |m| m.scale);
    // Warming: its first frame stands until the app's frames are steady,
    // then its clock starts (no app clock at all: at once).
    if intro.warming {
        if !warmth.is_none_or(|w| w.is_warm()) {
            return;
        }
        intro = Intro::live(now);
        o.intro = Some(intro);
        o.since = now + welcome_at(scale);
    }
    let t = intro.held.map_or((now - intro.start) as f32, |h| h);
    if intro.held.is_none() && t >= intro_len(scale) {
        end_intro(&mut o, &files, now, false);
        return;
    }
    // The sting: the pass's sweep with the bands, the reveal as the
    // lockup lands (the app's table, on the `ui` bus at its volume; silent
    // where the theme has no UI sound). Not when held or at scale 0.
    if sent.0 != intro.start {
        *sent = (intro.start, 0);
    }
    if let Some(mut cues) = cues.filter(|_| intro.held.is_none() && scale > 0.0) {
        for (i, (at, event)) in [(0.02, "pass_sweep"), (0.62, "end_reveal")].into_iter().enumerate() {
            if sent.1 as usize == i && t >= at * scale {
                cues.write(chimera_ui::UiCue::new(event));
                sent.1 += 1;
            }
        }
    }
}

/// A held intro's timelines stay at its time (screenshots).
fn hold_intro(o: Res<Onboarding>, clock: Res<UiClock>, mut parts: Query<&mut Timeline, With<IntroPart>>) {
    let Some(t) = o.intro.and_then(|i| i.still_at()) else { return };
    let start = clock.now - t as f64;
    for mut tl in &mut parts {
        if tl.start != Some(start) {
            tl.start = Some(start);
        }
    }
}

/// The timecode in the intro's corner, HH:MM:SS:FF at 30 frames.
fn intro_clock(o: Res<Onboarding>, clock: Res<UiClock>, mut texts: Query<&mut Text, With<IntroClock>>) {
    let Some(intro) = o.intro else { return };
    let t = intro.still_at().unwrap_or((clock.now - intro.start) as f32).max(0.0);
    let tc = timecode(t);
    for mut text in &mut texts {
        if text.0 != tc {
            text.0 = tc.clone();
        }
    }
}

/// `t` seconds as broadcast timecode at 30 frames a second.
pub fn timecode(t: f32) -> String {
    let frames = (t * 30.0).floor() as u32;
    let (ff, s) = (frames % 30, frames / 30);
    format!("00:{:02}:{:02}:{ff:02}", s / 60, s % 60)
}

/// What the intro's system check reads.
#[derive(Clone, Debug, PartialEq)]
struct Readout {
    /// The window's size in pixels, if there is one.
    display: Option<(u32, u32)>,
    /// Games installed, of the games the app knows (the mount doctor's).
    games: Option<(usize, usize)>,
}

/// The intro's nodes, rebuilt when it starts or the window changes.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_intro(
    o: Res<Onboarding>,
    words: Res<Words>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<IntroRoot>>,
    (theme, kit, units, motion): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>, Option<Res<Motion>>),
    (title, mounts, windows): (
        Option<Res<crate::esc_menu::EscMenuTitle>>,
        Option<Res<chimera_net::net::manifest::LocalMounts>>,
        Query<&Window, With<bevy::window::PrimaryWindow>>,
    ),
    clock: Res<UiClock>,
    mut shown: Local<Option<(Intro, (u32, u32), u32, Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let scale = motion.map_or(1.0, |m| m.scale);
    let cam = camera.iter().next();
    let key = o.intro.filter(|_| o.active).map(|i| (i, ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), (scale * 1000.0) as u32, cam));
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((intro, ..)) = key {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let readout = Readout {
            display: windows.iter().next().map(|w| (w.physical_width(), w.physical_height())),
            games: mounts.as_deref().map(|m| (m.0.games.iter().filter(|g| g.installed).count(), m.0.games.len())),
        };
        let app = title.map_or_else(|| crate::esc_menu::EscMenuTitle::default().0, |t| t.0.clone());
        // Held: every timeline starts `held` seconds ago (`hold_intro`
        // keeps it there).
        let start = intro.still_at().map_or(intro.start, |h| clock.now - h as f64);
        let r = draw_intro(&mut commands, &ui, &words, &app, &readout, start, scale);
        if let Some(cam) = cam {
            commands.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *shown = key;
}

/// One track, its times scaled by the motion setting.
fn tr(prop: Prop, from: f32, to: f32, at: f32, dur: f32, ease: Ease, s: f32) -> Track {
    Track::new(prop, from, to).at(at * s).dur(dur * s).ease(ease)
}

/// The intro's frame (module doc, "Intro", beat by beat). At motion scale
/// 0 it is its last frame, nothing moving.
fn draw_intro(c: &mut Commands, ui: &Ui, w: &Words, app: &str, readout: &Readout, start: f64, s: f32) -> Entity {
    let moving = s > 0.0;
    // Animates `e` with `tracks` (not at scale 0: the spawned state is the
    // end state).
    let put = |c: &mut Commands, e: Entity, tracks: Vec<Track>| {
        if moving {
            c.entity(e).insert((Timeline::new(tracks).started(start), IntroPart));
        }
        e
    };
    let fill = [("app", app)];
    let t = ui.theme;
    let root = c
        .spawn((
            IntroRoot,
            GlobalZIndex(ONBOARDING_Z + 1),
            UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, None]).bg(Tone::Ink),
            Node::default(),
            bevy::ui::FocusPolicy::Block,
            Interaction::None,
        ))
        .id();
    // The out: the whole frame fades into Welcome, whose entry plays under it.
    put(c, root, vec![tr(Prop::Opacity, 1.0, 0.0, INTRO_SECS - INTRO_OUT, INTRO_OUT, Ease::ExpoIn, s)]);
    // The ground: Welcome's own navy fall, so the fade out changes nothing
    // of it (no radial glows: they band).
    let wash = ui.boxed(c, root, UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, None]));
    c.entity(wash).insert(BackgroundGradient(vec![
        LinearGradient::new(
            162f32.to_radians(),
            vec![
                ColorStop::percent(t.tone_a(Tone::Hero, 0.16), 0.0),
                ColorStop::percent(t.tone_a(Tone::Hero, 0.05), 42.0),
                ColorStop::percent(t.tone_a(Tone::Hero, 0.0), 100.0),
            ],
        )
        .into(),
    ]));
    let (m, top, foot) = (4.6, 7.2, 4.4);
    // Beat 1 (0-0.4 s): the blueprint. Eleven hairlines flicker on from
    // the middle out, the registration marks at the corners.
    for k in 0..11 {
        let x = m + k as f32 * (100.0 - 2.0 * m) / 10.0;
        let l = ui.boxed(c, root, UiStyle { w: Some(0.06), ..UiStyle::column() }.abs([Some(x), Some(top), None, Some(foot)]).bg_a(Tone::AccText, 0.07));
        let at = 0.04 + (k as f32 - 5.0).abs() * 0.035;
        put(c, l, vec![tr(Prop::Opacity, 0.0, 1.0, at, 0.12, Ease::Steps(3), s)]);
    }
    for (l, tp, r, b) in [(Some(1.8), Some(1.8), None, None), (None, Some(1.8), Some(1.8), None), (Some(1.8), None, None, Some(1.8)), (None, None, Some(1.8), Some(1.8))] {
        let mk = ui.boxed(c, root, UiStyle { w: Some(1.3), h: Some(1.3), ..UiStyle::column() }.abs([l, tp, r, b]));
        ui.boxed(c, mk, UiStyle { w: Some(1.3), h: Some(0.06), ..UiStyle::row() }.abs([Some(0.0), Some(0.62), None, None]).bg(Tone::Fg3));
        ui.boxed(c, mk, UiStyle { w: Some(0.06), h: Some(1.3), ..UiStyle::row() }.abs([Some(0.62), Some(0.0), None, None]).bg(Tone::Fg3));
    }
    // The broadcast bar: the channel and the app at the left, ON AIR and
    // the timecode at the right, the rule under them drawn out from the
    // middle.
    let bar = ui.boxed(c, root, UiStyle { align: Some(AlignItems::Center), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() }.abs([Some(m), Some(2.6), Some(m), None]));
    put(c, bar, vec![tr(Prop::Opacity, 0.0, 1.0, 0.12, 0.16, Ease::Steps(2), s)]);
    let left = ui.boxed(c, bar, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
    ui.boxed(c, left, UiStyle { w: Some(0.5), h: Some(0.5), ..UiStyle::row() }.bg(Tone::Acc));
    ui.text(c, left, Role::Mono, 0.62, Tone::Fg3, &w.f("intro.channel", &fill));
    let right = ui.boxed(c, bar, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
    let dot = ui.boxed(c, right, UiStyle { w: Some(0.55), h: Some(0.55), ..UiStyle::row() }.bg(Tone::Threat));
    // The light blinks twice a second while it plays.
    let blink: Vec<Track> = (0..6).map(|i| tr(Prop::Opacity, if i % 2 == 0 { 1.0 } else { 0.25 }, if i % 2 == 0 { 0.25 } else { 1.0 }, 0.2 + i as f32 * 0.5, 0.02, Ease::Linear, s)).collect();
    put(c, dot, blink);
    ui.text(c, right, Role::Mono, 0.62, Tone::Threat, &w.t("intro.live"));
    let tc = ui.text(c, right, Role::Mono, 0.62, Tone::Fg2, &timecode(if moving { 0.0 } else { INTRO_SECS }));
    c.entity(tc).insert(IntroClock);
    let rule = ui.boxed(c, root, UiStyle { h: Some(0.06), ..UiStyle::row() }.abs([Some(m), Some(top), Some(m), None]).bg(Tone::Fg5));
    put(c, rule, vec![tr(Prop::ScaleX, 0.0, 1.0, 0.1, 0.6, Ease::ExpoOut, s)]);

    // Beat 2 (0.05-0.9 s): the logo pass. Four bands on the slant (an
    // accent hairline, the hero slab with the mark, an accent band, a light
    // band) sweep the frame, fast, slow while the slab stands over the
    // lockup, fast; the lockup comes up under it and its trailing edge
    // reveals it.
    let (lk, lock_y) = (INTRO_LK, 15.4);
    let lockup_row = ui.boxed(c, root, UiStyle { justify: Some(JustifyContent::Center), ..UiStyle::row() }.abs([Some(0.0), Some(lock_y), Some(0.0), None]));
    let parts = ui.lockup_parts(c, lockup_row, lk);
    // Beat 3 (0.4-1.0 s): the lockup assembles. The mark pops (on a back
    // ease) and flickers on; ULTIMATE tracks in from wide; GUN GAME
    // flickers on like a tube warming.
    put(
        c,
        parts.mark,
        vec![
            tr(Prop::ScaleX, 0.55, 1.0, 0.5, 0.5, Ease::Back(1.7), s),
            tr(Prop::ScaleY, 0.55, 1.0, 0.5, 0.5, Ease::Back(1.7), s),
            tr(Prop::Opacity, 0.0, 1.0, 0.4, 0.12, Ease::Steps(3), s),
        ],
    );
    let n = parts.ultimate.len() as f32;
    for (i, &e) in parts.ultimate.iter().enumerate() {
        let off = (i as f32 - (n - 1.0) / 2.0) * 1.4;
        put(c, e, vec![tr(Prop::X, off, 0.0, 0.5, 0.75, Ease::ExpoOut, s), tr(Prop::Opacity, 0.0, 1.0, 0.45 + i as f32 * 0.02, 0.25, Ease::Linear, s)]);
    }
    put(
        c,
        parts.wordmark,
        vec![
            tr(Prop::Opacity, 0.0, 0.85, 0.6, 0.04, Ease::Linear, s),
            tr(Prop::Opacity, 0.85, 0.12, 0.68, 0.03, Ease::Linear, s),
            tr(Prop::Opacity, 0.12, 1.0, 0.74, 0.05, Ease::Linear, s),
            tr(Prop::Opacity, 1.0, 0.45, 0.84, 0.02, Ease::Linear, s),
            tr(Prop::Opacity, 0.45, 1.0, 0.88, 0.06, Ease::Steps(4), s),
        ],
    );
    if moving {
        let height = 100.0 * 9.0 / 16.0 + 4.0;
        let lean = height * t.tan();
        // The hero slab (30 cq) stands over the lockup at the sweep's slow
        // middle (0.35-0.6 s): its left at 50 - 15 cq at mid-height.
        let (slab_x, slab_w) = (1.8, 30.0);
        let mid = 50.0 - slab_w / 2.0 - slab_x - lean / 2.0;
        let from = -40.0 - lean;
        let group = ui.boxed(c, root, UiStyle { w: Some(40.0 + lean), ..UiStyle::row() }.abs([Some(0.0), Some(-2.0), None, Some(-2.0)]));
        c.entity(group).insert(Pickable::IGNORE);
        put(c, group, vec![tr(Prop::X, from, 2.0 * mid - from, 0.05, 0.85, Ease::Hold, s)]);
        for (x, bw, tone) in [(0.0, 0.8, Tone::Acc), (slab_x, slab_w, Tone::Hero), (32.4, 5.0, Tone::Acc), (38.2, 1.4, Tone::Fg)] {
            ui.slant_at(c, group, chimera_ui::style::UiSlant::new(tone, 1.0), [Some(x), Some(0.0), None, None], bw + lean, height);
        }
        // The mark rides in the hero slab, as in the logo pass.
        let mh = 4.2;
        let ride = ui.boxed(c, group, UiStyle { w: Some(slab_w), h: Some(mh), justify: Some(JustifyContent::Center), ..UiStyle::row() }.abs([Some(1.8 + lean / 2.0), Some(height / 2.0 - mh / 2.0), None, None]));
        ui.kit_icon(c, ride, "mark", mh, chimera_ui::controls::MARK_ASPECT, Tone::OnHero);
    }

    // Beat 4 (0.9-1.5 s): the rule under the lockup scans out from the
    // middle, an accent tick riding it; the welcome comes up: an eyebrow,
    // "Welcome to <app>" rising, the tagline.
    let rule_y = lock_y + 2.05 * lk + 2.8;
    let under = ui.boxed(c, root, UiStyle { justify: Some(JustifyContent::Center), ..UiStyle::row() }.abs([Some(0.0), Some(rule_y), Some(0.0), None]));
    let line = ui.boxed(c, under, UiStyle { w: Some(46.0), h: Some(0.06), ..UiStyle::row() }.bg(Tone::Fg4));
    put(c, line, vec![tr(Prop::ScaleX, 0.0, 1.0, 0.9, 0.55, Ease::ExpoOut, s)]);
    let tick = ui.boxed(c, line, UiStyle { w: Some(4.0), h: Some(0.2), ..UiStyle::row() }.abs([Some(21.0), Some(-0.07), None, None]).bg(Tone::Acc));
    put(c, tick, vec![tr(Prop::X, -21.0, 0.0, 0.9, 0.7, Ease::ExpoOut, s), tr(Prop::Opacity, 0.0, 1.0, 0.9, 0.05, Ease::Linear, s)]);
    let col = ui.boxed(c, root, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::column() }.abs([Some(0.0), Some(rule_y + 1.9), Some(0.0), None]));
    let eyebrow = ui.text(c, col, Role::Mono, 0.7, Tone::AccText, &w.f("intro.eyebrow", &fill));
    put(c, eyebrow, vec![tr(Prop::Opacity, 0.0, 1.0, 0.98, 0.12, Ease::Steps(3), s)]);
    let title = ui.text(c, col, Role::DisplayBlack, 2.7, Tone::Fg, &w.f("intro.title", &fill));
    put(c, title, vec![tr(Prop::Y, 1.6, 0.0, 1.05, 0.6, Ease::ExpoOut, s), tr(Prop::Opacity, 0.0, 1.0, 1.05, 0.22, Ease::Linear, s)]);
    let tagline = ui.sentence(c, col, 1.0, Tone::Fg2, &w.f("intro.tagline", &fill));
    put(c, tagline, vec![tr(Prop::Y, 0.8, 0.0, 1.22, 0.5, Ease::ExpoOut, s), tr(Prop::Opacity, 0.0, 1.0, 1.22, 0.3, Ease::Linear, s)]);

    // Beat 5 (1.3-1.9 s): the system check types in at the bottom left
    // (display, audio, games found, account: none yet), the skip hint at
    // the right.
    let check = ui.boxed(c, root, UiStyle { gap: Some(0.55), ..UiStyle::column() }.abs([Some(m), None, None, Some(foot + 2.0)]));
    let head = ui.boxed(c, check, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
    ui.boxed(c, head, UiStyle { w: Some(1.6), h: Some(0.12), ..UiStyle::row() }.bg(Tone::Acc));
    ui.text(c, head, Role::Mono, 0.56, Tone::Fg3, &w.t("intro.check"));
    put(c, head, vec![tr(Prop::Opacity, 0.0, 1.0, 1.28, 0.1, Ease::Steps(2), s)]);
    let display = readout.display.map_or_else(|| w.t("intro.display_none"), |(x, y)| format!("{x} x {y}"));
    let games = match readout.games {
        Some((n, of)) if n > 0 => w.f("intro.games", &[("n", &n.to_string()), ("of", &of.to_string())]),
        _ => w.t("intro.games_none"),
    };
    let values = [(display, Tone::Fg), (w.t("intro.audio"), Tone::Fg), (games, Tone::Fg), (w.t("intro.account"), Tone::AccText)];
    for (i, (value, tone)) in values.iter().enumerate() {
        let row = ui.boxed(c, check, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
        let k = ui.boxed(c, row, UiStyle { w: Some(7.4), ..UiStyle::row() });
        ui.text(c, k, Role::Mono, 0.62, Tone::Fg3, &w.at("intro.rows", i));
        ui.text(c, row, Role::Mono, 0.62, *tone, value);
        let at = 1.36 + i as f32 * 0.12;
        put(c, row, vec![tr(Prop::Opacity, 0.0, 1.0, at, 0.1, Ease::Steps(2), s), tr(Prop::X, 0.8, 0.0, at, 0.3, Ease::ExpoOut, s)]);
    }
    let skip = ui.boxed(c, root, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([None, None, Some(m), Some(foot + 2.0)]));
    ui.text(c, skip, Role::Mono, 0.62, Tone::Fg3, &w.t("intro.skip"));
    ui.kbd(c, skip, &w.t("intro.skip_key"), 0.62, Tone::Fg2);
    put(c, skip, vec![tr(Prop::Opacity, 0.0, 1.0, 0.3, 0.2, Ease::Linear, s)]);

    // The tick bar along the foot runs out over the whole intro, into
    // Welcome.
    let bar = ui.boxed(c, root, UiStyle { h: Some(0.7), justify: Some(JustifyContent::SpaceBetween), align: Some(AlignItems::FlexEnd), ..UiStyle::row() }.abs([Some(m), None, Some(m), Some(foot)]));
    for i in 0..41 {
        let h = if i % 10 == 0 { 0.7 } else if i % 5 == 0 { 0.45 } else { 0.25 };
        ui.boxed(c, bar, UiStyle { w: Some(0.06), h: Some(h), ..UiStyle::row() }.bg(Tone::Fg4));
    }
    let full = 100.0 - 2.0 * m;
    let run = ui.boxed(c, bar, UiStyle { w: Some(full), h: Some(0.18), ..UiStyle::row() }.abs([Some(0.0), None, None, Some(0.0)]).bg(Tone::Acc));
    put(c, run, vec![tr(Prop::Width, 0.0, full, 0.0, INTRO_SECS - INTRO_OUT, Ease::Linear, s)]);
    root
}

// ---------------------------------------------------------------------------
// Staging.

/// `ui_scene onboarding <screen>` (module doc): each screen with the
/// staged hub, nothing written.
fn scene(w: &mut World, args: &[String]) -> Result<(), String> {
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    w.insert_resource(OnboardingBackend::staged());
    w.insert_resource(OnboardingFiles::default());
    let staged = StagedHub::default();
    let fp = "3f9a0b71c2d45e6f".repeat(4);
    let link = HubLink { host: "hub.example.net:27080".into(), fingerprint: Some(fp.clone()), code: Some("K7Q-M2P-9TR".into()) };
    let full = link.to_text(true);
    let mut o = Onboarding { active: true, since: now - 10.0, avatar: 13, ..Default::default() };
    let typed = |o: &mut Onboarding, text: &str| {
        o.input = text.into();
        o.caret = Caret::end_of(text);
    };
    let at_hub = |o: &mut Onboarding| {
        o.link = Some(link.clone());
        o.answer = Some(staged.answer("invite"));
    };
    let what = args.first().map(String::as_str).unwrap_or("welcome");
    match what {
        "off" => {
            w.resource_mut::<Onboarding>().active = false;
            return Ok(());
        }
        "welcome" => {}
        "intro" => {
            // Live from now (into Welcome), or held at `t` seconds.
            let held = match args.get(1) {
                Some(t) => Some(t.parse::<f32>().map_err(|_| format!("ui_scene onboarding intro {t}: seconds"))?.max(0.0)),
                None => None,
            };
            let scale = w.get_resource::<Motion>().map_or(1.0, |m| m.scale);
            o.intro = Some(Intro { held, ..Intro::live(now) });
            o.since = now + welcome_at(scale);
        }
        "clipboard" | "checked" => {
            typed(&mut o, &full);
            o.source = Source::Clipboard;
            o.parsed = InviteInput::parse(&full);
            if what == "checked" {
                at_hub(&mut o);
                o.valid = Some(full.clone());
            }
        }
        "checking" => {
            typed(&mut o, &full);
            o.link = Some(link.clone());
            o.step = Step::Checking;
            o.pending = Some(9999);
        }
        "used" | "unknown" | "unreachable" | "certificate" => {
            typed(&mut o, &full);
            o.parsed = InviteInput::parse(&full);
            o.link = Some(link.clone());
            o.problem = Some(match what {
                "used" => Problem::Used,
                "unknown" => Problem::Unknown,
                "unreachable" => Problem::Unreachable,
                _ => Problem::Certificate { offered: Some(short_fp(&"77d0e2c4a51b19af".repeat(4))) },
            });
        }
        "hub" => {
            at_hub(&mut o);
            o.step = Step::Hub;
        }
        "name" | "name-bad" | "name-taken" => {
            at_hub(&mut o);
            o.step = Step::Name;
            let (name, display) = match what {
                "name" => ("sam_rocket", "Sam Rocket"),
                "name-bad" => ("9 lives!", "9 Lives!"),
                _ => ("rocket", ""),
            };
            o.username = name.into();
            o.ucaret = Caret::end_of(name);
            o.display = display.into();
            o.asked_name = Some(name.into());
            o.name = staged.check(&link, "K7Q-M2P-9TR", Some(name)).ok().and_then(|a| a.username);
        }
        "key" | "key-saved" => {
            at_hub(&mut o);
            o.step = Step::Key;
            o.signed_in = Some(staged.file("sam_rocket", None));
            o.recovery = Some(STAGED_KEY.into());
            if what == "key-saved" {
                // A made-up place, a USB stick (nothing is written).
                let path = if cfg!(windows) { r"E:\keys\friday-night-hub-recovery-key.txt" } else { "/media/usb-stick/keys/friday-night-hub-recovery-key.txt" };
                o.saved_path = Some(path.into());
                o.said = Some(w.resource::<Words>().t("key.saved"));
            }
        }
        "have" | "recover" => {
            o.step = Step::Have;
            o.last_hub = Some(HubLink { code: None, ..link.clone() });
            if what == "recover" {
                o.tab = HaveTab::Recover;
                o.have_code = STAGED_KEY.into();
            } else {
                o.have_code = "H4K-9".into();
            }
        }
        "returning" | "offline" => {
            o.active = false;
            w.insert_resource(AccountShown(Some(Shown { display: "Sam Rocket".into(), username: "sam_rocket".into(), hub: "Friday Night Hub".into() })));
            let mut hub = chimera_net::net::hub::Hub::at("https://hub.example.net:27080", PathBuf::from("staged-account.toml"));
            if what == "offline" {
                hub.error = Some("the hub at https://hub.example.net:27080 didn't answer (staged)".into());
                hub.fetched = Some(std::time::Instant::now());
                hub.busy = true;
            }
            w.insert_resource(hub);
        }
        "done" => {
            o.active = false;
            o.toast = Some(("Welcome, Sam Rocket".into(), "You're in Friday Night Hub. This PC stays signed in.".into(), now));
        }
        other => {
            return Err(format!(
                "ui_scene onboarding {other}: intro [t], welcome, clipboard, checked, checking, used, unknown, unreachable, certificate, hub, name, \
                 name-bad, name-taken, key, key-saved, have, recover, done, returning, offline or off"
            ));
        }
    }
    *w.resource_mut::<Onboarding>() = o;
    Ok(())
}

/// Fills the welcome field from the clipboard at the first frame when it
/// holds a hub link, and checks it (as a paste).
pub fn from_clipboard(mut o: ResMut<Onboarding>, mut done: Local<bool>, shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>, mut own: Local<chimera_ui::clipboard::Clipboard>) {
    if *done || !o.active {
        return;
    }
    *done = true;
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let Some(text) = clipboard.get() else { return };
    let text = chimera_ui::clipboard::single_line(&text);
    if o.input.is_empty()
        && let Some(p) = InviteInput::parse(&text).filter(|p| p.kind == InputKind::Link)
    {
        o.input = text.trim().to_string();
        o.caret = Caret::end_of(&o.input);
        o.source = Source::Clipboard;
        o.parsed = Some(p);
        o.check_now = true;
    }
}

/// The game's window back in front (the player copied the invite in
/// another app): an invite link on the clipboard fills Welcome's field and
/// is checked, when the field is empty or holds something else that isn't
/// a checked invite (a checked one, or one being checked, is never
/// replaced).
pub fn refocus_clipboard(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut o: ResMut<Onboarding>,
    clock: Res<UiClock>,
    shared: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
    mut own: Local<chimera_ui::clipboard::Clipboard>,
) {
    if !focus.read().any(|f| f.focused) || !o.active || o.step != Step::Welcome || o.intro.is_some() || o.pending.is_some() || o.auto.is_some() {
        return;
    }
    let current = o.input.trim().to_string();
    if !current.is_empty() && o.valid.as_deref() == Some(current.as_str()) {
        return;
    }
    let clipboard = match shared {
        Some(c) => c.into_inner(),
        None => &mut *own,
    };
    let Some(text) = clipboard.get() else { return };
    let text = chimera_ui::clipboard::single_line(&text).trim().to_string();
    let Some(p) = InviteInput::parse(&text).filter(|p| p.kind == InputKind::Link) else { return };
    if text == current {
        return;
    }
    o.input = text;
    o.caret = Caret::end_of(&o.input);
    edited(&mut o, clock.now);
    o.source = Source::Clipboard;
    o.parsed = Some(p);
    o.check_now = true;
}
