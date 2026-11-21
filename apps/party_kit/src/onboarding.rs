//! The first run: from an invite to the main menu, signed in (feature
//! `present`; docs/plans/active/onboarding.md, the approved mockups and
//! the user's decisions of 2026-11-21). Drawn in the Signal look over the
//! main menu (`ONBOARDING_Z`), its words from `data/onboarding.toml`
//! (`Words`), its colours and sizes the theme's.
//!
//! - **When** (`maybe_start`, the app's start only: never a tool run or a
//!   test): the run starts at the main menu and this PC has no account,
//!   no legacy asset token and no skip mark (`OnboardingFiles`).
//! - **Welcome** (`Step::Welcome`): the one field takes the whole hub
//!   link, a web address with the code, or the bare code
//!   (`chimera_assets::account::InviteInput`), read on Enter; the pane
//!   "What an invite carries" lights each part found. A link already on
//!   the clipboard is filled in at the start (nothing is sent).
//!   "I already have an account" (`Step::Have`) and "Play without an
//!   account" (writes the skip mark) under it.
//! - **Checking**: `POST /accounts/check` on a thread (`OnboardingHub`);
//!   a wrong, used or withdrawn code, an unreachable hub and another
//!   certificate each say what happened, why and what to do (`Problem`),
//!   the input kept. A device link code goes straight to redeeming.
//! - **Hub**: "You're joining <name>", the hub's address, the certificate
//!   check, its players and its MOTD; "Not this one" goes back.
//! - **Name**: the username (the rules lit as typed,
//!   `chimera_assets::codes::UsernameRules`; availability asked of the
//!   hub 0.45 s after the last key, with free suggestions when taken), the
//!   display name (optional), the avatar (`cl_avatar`'s 32 looks; arrows
//!   move it while it has the focus) and a preview of how others see you.
//!   Continue redeems: the device key is made here.
//! - **Key**: the recovery key in groups, Copy (Ctrl+C), Save as file
//!   (`OnboardingFiles::key_dir`), "I saved it" (Space) unlocks Continue.
//! - **Have**: Link a device (a link code) or Recover with key (Q and E);
//!   the hub's link asked when this PC knows none. A recovery shows the
//!   new key (the old one stops working).
//! - **Done** (`finish`): the account file written and made live, the
//!   avatar set (`cl_avatar`), the hub and parties of the account taken
//!   up, and a toast on the main menu ("Welcome, <name>").
//! - **No blocking animations** (decision 8): each screen's parts slide in
//!   (`Timeline`, staggered, scaled by `ui_motion_scale`), but the field
//!   and buttons work from the first frame, and any key or click while
//!   they move ends the entry at once (`skip_entry`).
//! - The menus behind it take no keys while it shows (`takes_keys`).
//! - **Returning players** see no sign-in screen: the account's tickets
//!   come silently (`chimera_assets::account::keep_fresh`, the hub's
//!   `use_user_hub`). The main menu shows the **account chip** under the
//!   bar's rack (`show_chip`: the avatar, the display name, `@username ·
//!   <hub>` and a light, green while the hub answers, amber when its last
//!   call failed), and while the hub can't be reached a slim **offline
//!   banner** ("Can't reach <hub>", playing offline goes on, Retry now;
//!   it retries by itself every `RETRY_SECS`). `AccountShown` is the
//!   account's names (the app's start, or the first run's end).
//! - `ui_scene onboarding <welcome|clipboard|checking|used|unknown|
//!   unreachable|certificate|hub|name|name-bad|name-taken|key|have|
//!   recover|done|returning|offline|off>` stages each screen with a made-up hub
//!   (`StagedHub`), for screenshots.
//!
//! Known shortcuts: the first-launch intro (decision 7) isn't built
//! (the plan's next step); clicking an invite link doesn't open the game
//! yet (no link handler registered); the screens rebuild whole on any
//! change; no keyboard focus between the buttons (Enter is the hero,
//! Esc goes back); the chip opens nothing yet (no Profile screen: rename,
//! link a device and sign out are the hub's calls, `account::rename`, not
//! drawn yet).

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
    Role, Tone, UiStyle,
    avatar::{AvatarLook, AvatarMarks, Face},
    controls::{HERO_H, Kind, NOTE, SENTENCE, State, Ui, clickable},
    motion::{Ease, Motion, Prop, Timeline, Track, UiClock},
    text_edit::{Caret, text_key},
};

use crate::main_menu::MenuUiCamera;

/// Over the main menu and Settings, under the party panel, the report
/// dialog and the console.
pub const ONBOARDING_Z: i32 = 93;
/// Seconds between the offline banner's own retries.
pub const RETRY_SECS: f64 = 30.0;
/// The screens' words.
pub const ONBOARDING_TOML: &str = include_str!("../data/onboarding.toml");
/// Seconds after the last key before a username is asked of the hub.
pub const NAME_PAUSE: f64 = 0.45;
/// Seconds the welcome toast stays on the main menu.
pub const TOAST_SECS: f64 = 6.0;
/// The entry's length and its stagger, seconds at motion scale 1.
const ENTRY_SECS: f32 = 0.52;
const STAGGER: f32 = 0.055;

// ---------------------------------------------------------------------------
// The words.

/// `data/onboarding.toml`: `t("welcome.title")`; a missing key shows as
/// `[welcome.title]` (a test looks for those).
#[derive(Resource, Clone, Debug)]
pub struct Words(toml::Table);

impl Default for Words {
    fn default() -> Self {
        Words(toml::from_str(ONBOARDING_TOML).expect("data/onboarding.toml parses"))
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
    /// The field as read at the last Enter.
    pub parsed: Option<InviteInput>,
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
}

impl OnboardingFiles {
    /// The user's (beside `account.toml`; the key in Documents).
    pub fn user() -> Self {
        let account = AccountFile::path();
        let skip_mark = account.as_ref().map(|p| p.with_file_name("onboarding-skipped"));
        let key_dir = dirs::document_dir().or_else(|| account.as_ref().and_then(|p| p.parent().map(PathBuf::from)));
        Self { account, skip_mark, key_dir }
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
}

#[derive(Component)]
pub struct OnboardingRoot;

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
        app.add_systems(Update, (from_clipboard, clicks, keys, poll, name_pause, act, show, show_toast).chain())
            .add_systems(Update, (retry_hub, show_chip).chain());
    }
}

/// The app's start: the first run when this PC has no account, no legacy
/// token and no skip mark (module doc).
pub fn maybe_start(mut o: ResMut<Onboarding>, mut files: ResMut<OnboardingFiles>, mut shown: ResMut<AccountShown>, clock: Option<Res<UiClock>>) {
    *files = OnboardingFiles::user();
    if let Some(file) = files.account.as_ref().and_then(|p| AccountFile::load_from(p).ok().flatten()) {
        shown.0 = Some(Shown::of(&file, "the hub"));
    }
    let skipped = files.skip_mark.as_ref().is_some_and(|p| p.exists());
    let token = chimera_assets::access::Access::load().token.is_some();
    if account::has_account() || token || skipped {
        return;
    }
    o.active = true;
    o.go(Step::Welcome, clock.map_or(0.0, |c| c.now));
    o.avatar = rand_avatar();
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
/// Space, the avatar's arrows; any key ends a screen's entry.
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
) {
    if !o.active || console.is_some_and(|c| c.open) {
        events.clear();
        return;
    }
    if let Some(mut g) = guard {
        g.hold();
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
        skip_entry(&mut o, now);
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
                if caret.key(&mut o.input, tk, clipboard, 512, false) {
                    o.parsed = None;
                    o.problem = None;
                    o.source = Source::Typed;
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

/// Ends the screen's entry (any key or click while it plays).
fn skip_entry(o: &mut Onboarding, now: f64) {
    if now - o.since < 2.0 {
        o.since = now - 10.0;
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
            Reply::Checked(r, None) if ours => {
                o.pending = None;
                match r {
                    Ok(a) if a.kind == "invite" => {
                        o.answer = Some(a);
                        o.go(Step::Hub, now);
                    }
                    Ok(a) => {
                        // A link code or a re-invite: an account already, no name to choose.
                        o.answer = Some(a);
                        if let (Some(link), Some(code)) = (o.link.clone(), o.link.as_ref().and_then(|l| l.code.clone())) {
                            let request = RedeemRequest { code, device_label: account::device_label(), ..Default::default() };
                            ask(&mut o, &backend, &answers, Ask::Redeem { link, request }, false);
                        }
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
        skip_entry(&mut o, now);
        if o.pending.is_some() && !matches!(a, ObAction::Back) {
            continue;
        }
        match (a, o.step) {
            (ObAction::Clear, _) => {
                o.input.clear();
                o.caret = Caret::default();
                o.parsed = None;
                o.problem = None;
                o.source = Source::Typed;
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
            (ObAction::Continue | ObAction::Retry, Step::Welcome) => {
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
                        o.said = Some(words.f("key.saved", &[("name", &name)]));
                    }
                    Err(e) => o.said = Some(format!("{}: {e}", path.display())),
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

type ScreenKey = (Onboarding, (u32, u32), Option<Entity>);

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    o: Res<Onboarding>,
    words: Res<Words>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<OnboardingRoot>>,
    (theme, kit, units, motion): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>, Option<Res<Motion>>),
    (mut images, mut icons): (Option<ResMut<Assets<Image>>>, Option<ResMut<chimera_ui::icons::IconImages>>),
    mut shown: Local<Option<ScreenKey>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let cam = camera.iter().next();
    let key = o.active.then(|| {
        let mut snap = o.clone();
        snap.toast = None;
        snap.typed_at = 0.0;
        snap.asked_name = None;
        (snap, ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), cam)
    });
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if key.is_some() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let scale = motion.map_or(1.0, |m| m.scale);
        let mut d = Draw { ui: &ui, w: &words, o: &o, since: o.since, scale, i: 0, images: images.as_deref_mut(), icons: icons.as_deref_mut() };
        let r = d.screen(&mut commands);
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
                UiStyle { gap: Some(1.4), pad: Some([0.0, 2.4, 0.0, 1.6].into()), h: Some(3.6), ..UiStyle::row() }
                    .abs([Some(36.0), Some(9.4), None, None])
                    .bg(Tone::Hero),
                Node::default(),
                Pickable::IGNORE,
            ))
            .id();
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

/// The account chip and the offline banner on the main menu (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_chip(
    shown_account: Res<AccountShown>,
    o: Res<Onboarding>,
    hub: Option<Res<chimera_net::net::hub::Hub>>,
    avatar: Option<Res<chimera_client::client::profile::AvatarSetting>>,
    menu: Option<Res<crate::main_menu::PartyMenu>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<ChipRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    (mut images, mut icons): (Option<ResMut<Assets<Image>>>, Option<ResMut<chimera_ui::icons::IconImages>>),
    mut shown: Local<Option<(Shown, bool, String, i32, (u32, u32), Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let on_menu = menu.as_deref().is_some_and(|m| m.screen == crate::main_menu::Screen::MainMenu && m.page == crate::main_menu::MenuPage::Main);
    let cam = camera.iter().next();
    let key = shown_account.0.clone().filter(|_| on_menu && !o.active && crate::esc_menu::signal(Some(theme))).map(|a| {
        let offline = hub.as_deref().is_some_and(|h| h.offline());
        let hub_name = hub.as_deref().filter(|h| h.doc.is_some()).map(|h| h.name()).unwrap_or_else(|| a.hub.clone());
        (a, offline, hub_name, avatar.as_deref().map_or(-1, |s| s.0), ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), cam)
    });
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((a, offline, hub_name, look, _, cam)) = key.clone() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let c = &mut commands;
        let r = c
            .spawn((ChipRoot, GlobalZIndex(ONBOARDING_Z - 1), UiStyle::column().abs([Some(0.0), Some(0.0), None, None]).w_pct(100.0).h_pct(100.0), Node::default(), Pickable::IGNORE))
            .id();
        // The chip, under the rack at the right.
        let chip = ui.boxed(c, r, UiStyle { gap: Some(1.2), h: Some(4.2), pad: Some([0.0, 1.2, 0.0, 0.4].into()), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([None, Some(8.2), Some(4.6), None]).bg_a(Tone::Ink2, 0.8).outline(if offline { Tone::Fg4 } else { Tone::AccText }, 1.0));
        let byte = u8::try_from(look).ok().and_then(AvatarLook::from_byte).unwrap_or_else(|| AvatarLook::for_name(&a.username));
        match (images.as_deref_mut(), icons.as_deref_mut()) {
            (Some(images), Some(icons)) => {
                chimera_ui::avatar::spawn(&ui, c, chip, images, icons, Face::Look(byte), AvatarMarks { idle: offline, ..default() }, 3.4);
            }
            _ => {
                let e = ui.boxed(c, chip, UiStyle { w: Some(3.4), h: Some(3.4), ..UiStyle::row() });
                c.entity(e).insert(BackgroundColor(byte.ground()));
            }
        }
        let names = ui.boxed(c, chip, UiStyle { gap: Some(0.45), ..UiStyle::column() });
        ui.text_plain(c, names, Role::DisplayBold, 1.02, Tone::Fg, &a.display);
        let sub = ui.boxed(c, names, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, sub, if offline { Tone::Warn } else { Tone::Good }, 0.7, 0.7);
        let line = if offline { format!("Offline · {hub_name}") } else { format!("@{} · {hub_name}", a.username) };
        ui.text_plain(c, sub, Role::Mono, 0.56, if offline { Tone::Warn } else { Tone::Fg3 }, &line.to_uppercase());
        if offline {
            let b = ui.boxed(c, r, UiStyle { gap: Some(1.6), pad: Some([1.0, 1.6].into()), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([Some(36.0), Some(13.4), Some(4.6), None]).bg_a(Tone::Ink2, 0.94).outline(Tone::Warn, 0.22));
            c.entity(b).insert(bevy::ui::FocusPolicy::Block);
            ui.mark(c, b, Tone::Warn, 0.7, 0.7);
            ui.text(c, b, Role::DisplayBold, 0.92, Tone::Warn, &format!("Can't reach {hub_name}"));
            let s = ui.boxed(c, b, UiStyle { grow: Some(1.0), ..UiStyle::column() });
            ui.paragraph(c, s, Role::Text, 0.86, Tone::Fg2, "Playing offline: LAN and direct connect still work. Parties, the server list and downloads come back when it does.");
            let retry = ui.button(c, b, Kind::Secondary, "Retry now", None, 2.4, State::NONE);
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
    since: f64,
    scale: f32,
    i: u32,
    images: Option<&'b mut Assets<Image>>,
    icons: Option<&'b mut chimera_ui::icons::IconImages>,
}

impl Draw<'_, '_> {
    /// `e` slides in with the next stagger (decision 8: never blocking).
    fn anim(&mut self, c: &mut Commands, e: Entity) -> Entity {
        let i = self.i;
        self.i += 1;
        if self.scale > 0.0 {
            let at = (0.12 + i as f32 * STAGGER) * self.scale;
            let t = Timeline::new([Track::new(Prop::Y, 1.2, 0.0).at(at).dur(ENTRY_SECS * self.scale).ease(Ease::ExpoOut)]).started(self.since);
            c.entity(e).insert(t);
        }
        e
    }

    fn abs(&self, c: &mut Commands, parent: Entity, left: f32, top: f32, w: f32) -> Entity {
        self.ui.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.9), ..UiStyle::column() }.abs([Some(left), Some(top), None, None]))
    }

    fn screen(&mut self, c: &mut Commands) -> Entity {
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
        // The ground: a navy wash and the registration marks.
        ui.boxed(c, root, UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, None]).bg_a(Tone::Hero, 0.08));
        for (l, t, r, b) in [(Some(1.8), Some(1.8), None, None), (None, Some(1.8), Some(1.8), None), (Some(1.8), None, None, Some(1.8)), (None, None, Some(1.8), Some(1.8))] {
            let m = ui.boxed(c, root, UiStyle { w: Some(1.3), h: Some(1.3), ..UiStyle::column() }.abs([l, t, r, b]));
            ui.boxed(c, m, UiStyle { w: Some(1.3), h: Some(0.06), ..UiStyle::row() }.abs([Some(0.0), Some(0.62), None, None]).bg(Tone::Fg3));
            ui.boxed(c, m, UiStyle { w: Some(0.06), h: Some(1.3), ..UiStyle::row() }.abs([Some(0.62), Some(0.0), None, None]).bg(Tone::Fg3));
        }
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
        let bar = ui.boxed(c, root, UiStyle { align: Some(AlignItems::Center), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() }.abs([Some(4.6), Some(2.9), Some(4.6), None]));
        let brand = ui.boxed(c, bar, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
        let mark = ui.boxed(c, brand, UiStyle { gap: Some(0.18), ..UiStyle::row() });
        ui.mark(c, mark, Tone::Hero, 0.5, 1.9);
        ui.mark(c, mark, Tone::Fg, 0.5, 1.9);
        ui.mark(c, mark, Tone::Fg3, 0.5, 1.9);
        let wm = ui.boxed(c, brand, UiStyle { gap: Some(0.35), ..UiStyle::column() });
        ui.text(c, wm, Role::DisplayBlack, 1.25, Tone::Fg, &self.w.t("brand"));
        ui.text(c, wm, Role::Mono, 0.55, Tone::Fg3, tag);
        if let Some(now) = step {
            let steps = ui.boxed(c, bar, UiStyle { gap: Some(0.4), ..UiStyle::row() });
            for (i, name) in self.w.list("steps").iter().enumerate() {
                let s = ui.boxed(c, steps, UiStyle { w: Some(6.2), gap: Some(0.45), ..UiStyle::column() });
                let tone = if i < now { Tone::Fg } else if i == now { Tone::Acc } else { Tone::Fg4 };
                ui.mark(c, s, tone, 5.8, 0.34);
                ui.text(c, s, Role::Mono, 0.6, if i <= now { Tone::Fg } else { Tone::Fg3 }, name);
            }
        } else {
            let back = ui.boxed(c, bar, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
            c.entity(back).insert((clickable(), ObAction::Back));
            ui.text(c, back, Role::Mono, 0.62, Tone::Fg2, &format!("‹ {}", self.w.t("have.back")));
            ui.kbd(c, back, "Esc", 0.62, Tone::Fg3);
        }
        let rule = ui.boxed(c, root, UiStyle { h: Some(0.06), ..UiStyle::row() }.abs([Some(4.6), Some(7.2), Some(4.6), None]).bg(Tone::Fg5));
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
        self.anim(c, e);
    }

    fn h1(&mut self, c: &mut Commands, col: Entity, text: &str, size: f32) {
        let e = self.ui.text(c, col, Role::DisplayBlack, size, Tone::Fg, text);
        self.anim(c, e);
    }

    fn lead(&mut self, c: &mut Commands, col: Entity, text: &str) {
        let b = self.ui.boxed(c, col, UiStyle { max_w: Some(38.0), ..UiStyle::column() });
        self.ui.paragraph(c, b, Role::Text, 1.02, Tone::Fg2, text);
        self.anim(c, b);
    }

    /// A field: its label line, the kit's text field, a status line.
    #[allow(clippy::too_many_arguments)]
    fn field(
        &mut self,
        c: &mut Commands,
        col: Entity,
        label: &str,
        label_tone: Tone,
        right: Option<&str>,
        value: &str,
        placeholder: &str,
        caret: Option<Caret>,
        w: f32,
        st: State,
        status: Option<(Tone, &str)>,
        prefix: Option<&str>,
    ) -> Entity {
        let ui = self.ui;
        let f = ui.boxed(c, col, UiStyle { gap: Some(0.6), w: Some(w), ..UiStyle::column() });
        let head = ui.boxed(c, f, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() });
        ui.text(c, head, Role::Mono, 0.56, label_tone, label);
        if let Some(r) = right {
            ui.text(c, head, Role::Mono, 0.56, Tone::Fg3, r);
        }
        let line = ui.boxed(c, f, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
        let mut fw = w;
        if let Some(p) = prefix {
            ui.text_plain(c, line, Role::Text, 1.1, Tone::Fg3, p);
            fw -= 1.6;
        }
        let tf = ui.text_field(c, line, value, placeholder, caret.map(|k| k.cursor(value)), false, fw, st);
        if let Some((tone, s)) = status {
            let row = ui.boxed(c, f, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.mark(c, row, tone, 0.6, 0.6);
            ui.text_plain(c, row, Role::Mono, 0.62, if tone == Tone::Fg4 { Tone::Fg3 } else { tone }, &s.to_uppercase());
        }
        self.anim(c, f);
        tf
    }

    #[allow(clippy::too_many_arguments)]
    fn button(&mut self, c: &mut Commands, row: Entity, kind: Kind, label: &str, key: Option<&str>, a: ObAction, st: State) -> Entity {
        let h = if kind == Kind::Primary { HERO_H } else { 2.6 };
        let b = self.ui.button(c, row, kind, label, key, h, st);
        if !st.disabled {
            c.entity(b).insert(a);
        }
        b
    }

    fn btns(&mut self, c: &mut Commands, col: Entity, top: f32) -> Entity {
        let r = self.ui.boxed(c, col, UiStyle { gap: Some(1.1), align: Some(AlignItems::Center), margin: Some([top, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        self.anim(c, r);
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
        self.anim(c, a);
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
            let rows = self.ui.kv(
                c,
                col,
                &[
                    (&w.t("errors.invite_says"), short_fp(&expected)),
                    (&w.t("errors.hub_showed"), offered.clone().unwrap_or_else(|| "?".into())),
                ],
                11.0,
            );
            c.entity(rows).insert(Node { width: self.ui.units.val(36.0), margin: UiRect::top(self.ui.units.val(1.2)), ..default() });
            self.anim(c, rows);
        }
    }

    fn welcome(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.abs(c, root, 8.4, 11.6, 42.0);
        self.eyebrow(c, col, &w.t("welcome.eyebrow"));
        self.h1(c, col, &w.t("welcome.title"), 3.3);
        if matches!(o.problem, None | Some(Problem::NotInvite | Problem::NeedsHub)) {
            self.lead(c, col, &w.t("welcome.lead"));
            ui.boxed(c, col, UiStyle { h: Some(0.8), ..UiStyle::column() });
        }
        let (label, label_tone) = match o.source {
            Source::Clipboard => (w.t("welcome.label_clipboard"), Tone::AccText),
            Source::Opened => (w.t("welcome.label_opened"), Tone::AccText),
            Source::Typed => (w.t("welcome.label"), Tone::Fg3),
        };
        let bad = matches!(o.problem, Some(Problem::NotInvite | Problem::NeedsHub));
        let status_text;
        let status = match (&o.problem, &o.parsed) {
            (Some(Problem::NotInvite), _) => (Tone::Threat, w.t("welcome.not_invite")),
            (Some(Problem::NeedsHub), _) => (Tone::Warn, w.t("welcome.needs_hub")),
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
        let (value, caret) = field_view(&o.input, o.caret, o.parsed.as_ref());
        let tf = self.field(c, col, &label, label_tone, Some("Ctrl+V"), &value, &w.t("welcome.placeholder"), caret, 42.0, st, Some((status.0, &status_text)), None);
        c.entity(tf).insert(clickable());
        self.problem(c, col);
        let row = self.btns(c, col, 2.2);
        self.button(c, row, Kind::Primary, &w.t("welcome.continue"), Some("Enter"), ObAction::Continue, State::focused(true));
        self.button(c, row, Kind::Secondary, &w.t("welcome.have_account"), None, ObAction::Have, State::NONE);
        // Play without an account: tertiary, but plainly a choice (decision 9).
        let skip = ui.boxed(c, root, UiStyle { gap: Some(1.2), align: Some(AlignItems::Center), ..UiStyle::row() }.abs([Some(8.4), None, None, Some(4.4)]));
        self.button(c, skip, Kind::Ghost, &w.t("welcome.skip"), None, ObAction::Skip, State::NONE);
        ui.text(c, skip, Role::Mono, 0.58, Tone::Fg3, &w.t("welcome.skip_note"));
        self.anim(c, skip);
        self.carries(c, root);
        self.keys(c, root, &[("Enter", "Check".into()), ("Esc", "Clear".into())]);
    }

    /// The pane: what an invite carries, each part lit as it is found.
    fn carries(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let pane = self.abs(c, root, 58.0, 11.6, 33.6);
        let bp = ui.boxed(c, pane, UiStyle { gap: Some(0.0), pad: Some([2.0, 2.2].into()), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.75).outline(Tone::AccText, 0.22));
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
                    (Some(h), _) => (Some((Tone::Acc, "Found")), h.clone()),
                    (None, Some(h)) => (Some((Tone::Warn, "Last used")), h.clone()),
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
        let note = ui.boxed(c, pane, UiStyle { margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("welcome.pane_note"));
        self.anim(c, pane);
    }

    fn checking(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.abs(c, root, 8.4, 11.6, 42.0);
        self.eyebrow(c, col, &w.t("welcome.eyebrow"));
        self.h1(c, col, &w.t("checking.title"), 3.3);
        ui.boxed(c, col, UiStyle { h: Some(1.6), ..UiStyle::column() });
        let (value, _) = field_view(&o.input, o.caret, InviteInput::parse(&o.input).as_ref());
        self.field(c, col, &w.t("welcome.label"), Tone::Fg3, None, &value, "", None, 42.0, State { disabled: true, ..State::NONE }, None, None);
        let bar = ui.boxed(c, col, UiStyle { margin: Some([2.2, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.progress(c, bar, chimera_ui::controls::Progress::Connecting, &w.t("checking.connecting"), 30.0, 1.7, 0.62, true);
        self.anim(c, bar);
        let host = o.host();
        let rows = [(w.at("checking.rows", 0), host), (w.at("checking.rows", 1), "Comparing with the invite".to_string()), (w.at("checking.rows", 2), "Waiting".to_string())];
        let pairs: Vec<(&str, String)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let kv = ui.kv(c, col, &pairs, 11.0);
        c.entity(kv).insert(Node { width: ui.units.val(30.0), margin: UiRect::top(ui.units.val(1.8)), ..default() });
        self.anim(c, kv);
        let row = self.btns(c, col, 2.4);
        self.button(c, row, Kind::Secondary, &w.t("checking.cancel"), Some("Esc"), ObAction::Back, State::NONE);
    }

    fn hub(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let a = o.answer.clone().unwrap_or_default();
        let col = self.abs(c, root, 8.4, 13.4, 44.0);
        self.eyebrow(c, col, &w.t("hub.eyebrow"));
        let t = ui.text(c, col, Role::Mono, 0.8, Tone::Fg2, &w.t("hub.joining"));
        c.entity(t).insert(Node { margin: UiRect::top(ui.units.val(2.4)), ..default() });
        self.anim(c, t);
        self.h1(c, col, &a.hub.name, 4.6);
        let pinned = o.link.as_ref().is_some_and(|l| l.fingerprint.is_some());
        let pairs = [
            (w.at("hub.rows", 0), o.host()),
            (w.at("hub.rows", 1), if pinned { w.t("hub.matches") } else { w.t("hub.pinned_none") }),
            (w.at("hub.rows", 2), a.hub.players.to_string()),
        ];
        let pairs: Vec<(&str, String)> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let kv = ui.kv(c, col, &pairs, 11.0);
        c.entity(kv).insert(Node { width: ui.units.val(40.0), margin: UiRect::top(ui.units.val(2.2)), ..default() });
        self.anim(c, kv);
        let row = self.btns(c, col, 2.4);
        self.button(c, row, Kind::Primary, &w.f("hub.join", &[("hub", &a.hub.name)]), Some("Enter"), ObAction::Continue, State::focused(true));
        self.button(c, row, Kind::Secondary, &w.t("hub.not_this"), None, ObAction::Back, State::NONE);
        // The MOTD plate.
        let pane = self.abs(c, root, 60.0, 17.4, 31.6);
        let plate = ui.boxed(c, pane, UiStyle { gap: Some(1.2), pad: Some([2.2, 2.4].into()), ..UiStyle::column() }.bg(Tone::Plate).outline(Tone::Fg5, 1.0));
        ui.boxed(c, plate, UiStyle { h: Some(0.3), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), Some(0.0), None]).bg(Tone::Fg));
        let h = ui.boxed(c, plate, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, h, Tone::Acc, 0.9, 1.4);
        ui.text(c, h, Role::Mono, 0.62, Tone::Fg, &w.t("hub.motd"));
        let motd = if a.hub.motd.trim().is_empty() { w.t("hub.no_motd") } else { a.hub.motd.clone() };
        ui.paragraph(c, plate, Role::Text, 1.12, Tone::Fg, &motd);
        let note = ui.boxed(c, pane, UiStyle { margin: Some([2.0, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("hub.note"));
        self.anim(c, pane);
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
        let col = self.abs(c, root, 8.4, 10.6, 42.0);
        self.eyebrow(c, col, &w.t("name.eyebrow"));
        self.h1(c, col, &w.t("name.title"), 3.3);
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
        let st = State { focus: o.field == NameField::Username, error: !folded.is_empty() && (!rules.all() || taken), ..State::NONE };
        let uf = self.field(c, col, &w.t("name.username"), Tone::Fg3, Some(&count), &o.username, "", (o.field == NameField::Username).then_some(o.ucaret), 42.0, st, Some((tone, &status)), Some("@"));
        c.entity(uf).insert((clickable(), ObAction::Focus(NameField::Username)));
        // The rules, lit as typed.
        let grid = ui.boxed(c, col, UiStyle { gap: Some(0.6), margin: Some([1.0, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        let met = [rules.length, rules.characters, rules.starts_with_letter, rules.all() && o.name_ok()];
        for pair in [[0, 1], [2, 3]] {
            let r = ui.boxed(c, grid, UiStyle { gap: Some(2.0), ..UiStyle::row() });
            for i in pair {
                let cell = ui.boxed(c, r, UiStyle { w: Some(20.0), gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
                let lit = if folded.is_empty() || (i == 3 && (checking || !rules.all())) {
                    Tone::Fg4
                } else if met[i] {
                    Tone::Acc
                } else {
                    Tone::Threat
                };
                ui.mark(c, cell, lit, 0.6, 0.6);
                ui.text_plain(c, cell, Role::Mono, 0.6, if lit == Tone::Fg4 { Tone::Fg3 } else if lit == Tone::Acc { Tone::AccText } else { Tone::Threat }, &w.at("name.rules", i));
            }
        }
        self.anim(c, grid);
        if taken {
            let free = ui.boxed(c, col, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), margin: Some([1.0, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
            ui.text(c, free, Role::Mono, 0.56, Tone::Fg3, &w.t("name.free"));
            for s in o.name.as_ref().map(|n| n.suggestions.clone()).unwrap_or_default() {
                let b = ui.boxed(c, free, UiStyle { h: Some(2.2), pad: Some([0.0, 1.0].into()), align: Some(AlignItems::Center), ..UiStyle::row() }.outline(Tone::Fg3, 1.0));
                c.entity(b).insert((clickable(), ObAction::Suggest(s.clone())));
                ui.text_plain(c, b, Role::Mono, 0.72, Tone::Fg, &s);
            }
            self.anim(c, free);
        }
        let df = self.field(
            c,
            col,
            &w.t("name.display"),
            Tone::Fg3,
            Some(&w.t("name.optional")),
            &o.display,
            if folded.is_empty() { "" } else { &folded },
            (o.field == NameField::Display).then_some(o.dcaret),
            42.0,
            State::focused(o.field == NameField::Display),
            None,
            None,
        );
        c.entity(df).insert((clickable(), ObAction::Focus(NameField::Display)));
        let note = ui.boxed(c, col, UiStyle { margin: Some([1.0, 0.0, 0.0, 0.0].into()), max_w: Some(42.0), ..UiStyle::column() });
        ui.paragraph(c, note, Role::Text, NOTE, Tone::Fg3, &w.t("name.note"));
        self.anim(c, note);
        self.problem(c, col);
        let row = self.btns(c, col, 1.4);
        let ready = o.name_ok() && o.pending.is_none();
        self.button(c, row, Kind::Primary, &w.t("name.continue"), Some("Enter"), ObAction::Continue, State { focus: ready, disabled: !ready, ..State::NONE });
        self.button(c, row, Kind::Ghost, &w.t("name.back"), None, ObAction::Back, State::NONE);
        // The preview: how others see you.
        let shown = if o.display.trim().is_empty() { if folded.is_empty() { "...".to_string() } else { folded.clone() } } else { o.display.trim().to_string() };
        let pane = self.abs(c, root, 58.0, 14.4, 33.6);
        let bp = ui.boxed(c, pane, UiStyle { gap: Some(0.8), pad: Some([2.0, 2.2].into()), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.75).outline(Tone::AccText, 0.22));
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
        ui.text(c, bp, Role::Mono, 0.56, Tone::Fg3, &w.t("name.profile"));
        let kv = ui.kv(c, bp, &[("Username", format!("@{}", if folded.is_empty() { "..." } else { &folded })), ("Hub", hub.clone())], 9.0);
        let _ = kv;
        // The avatar: the 32 looks (decision 10).
        let av = ui.boxed(c, bp, UiStyle { gap: Some(0.6), margin: Some([1.0, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
        ui.text(c, av, Role::Mono, 0.56, if o.field == NameField::Avatar { Tone::AccText } else { Tone::Fg3 }, &w.t("name.avatar"));
        for row in 0..4u8 {
            let r = ui.boxed(c, av, UiStyle { gap: Some(0.35), ..UiStyle::row() });
            for col_i in 0..8u8 {
                let b = row * 8 + col_i;
                let sel = b == o.avatar;
                let cell = ui.boxed(c, r, UiStyle { pad: Some([0.15, 0.15].into()), ..UiStyle::row() }.outline(if sel { if o.field == NameField::Avatar { Tone::Acc } else { Tone::Fg } } else { Tone::Fg5 }, if sel { 2.0 } else { 1.0 }));
                c.entity(cell).insert((clickable(), ObAction::Avatar(b)));
                let a = self.avatar(c, cell, b, 1.7);
                c.entity(a).insert(Pickable::IGNORE);
            }
        }
        self.anim(c, pane);
        self.keys(c, root, &[("Tab", "Next field".into()), ("Enter", "Continue".into())]);
    }

    fn key(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let user = o.signed_in.as_ref().map(|f| f.username.clone()).unwrap_or_default();
        let hub = o.hub_name();
        let col = self.abs(c, root, 8.4, 11.6, 46.0);
        self.eyebrow(c, col, &w.t("key.eyebrow"));
        self.h1(c, col, &w.t("key.title"), 2.7);
        self.lead(c, col, &w.t("key.lead"));
        let label = w.f("key.label", &[("user", &user), ("hub", &hub)]);
        let kb = ui.boxed(c, col, UiStyle { gap: Some(0.8), pad: Some([1.5, 1.6].into()), margin: Some([2.0, 0.0, 0.0, 0.0].into()), ..UiStyle::column() }.bg_a(Tone::Fg, 0.04).outline(Tone::Fg4, 1.0));
        ui.text(c, kb, Role::Mono, 0.56, Tone::Fg3, &label);
        let groups = ui.boxed(c, kb, UiStyle { gap: Some(1.0), ..UiStyle::row() });
        let key = o.recovery.clone().unwrap_or_default();
        for (i, g) in key.split('-').enumerate() {
            if i > 0 {
                ui.text_plain(c, groups, Role::Mono, 1.7, Tone::Fg3, "–");
            }
            ui.text_plain(c, groups, Role::Mono, 1.7, Tone::Fg, g);
        }
        self.anim(c, kb);
        let row = self.btns(c, col, 1.6);
        self.button(c, row, Kind::Secondary, &w.t("key.copy"), Some("Ctrl+C"), ObAction::Copy, State::NONE);
        self.button(c, row, Kind::Secondary, &w.t("key.save"), None, ObAction::Save, State::NONE);
        if let Some(s) = &o.said {
            let r = ui.boxed(c, row, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
            ui.mark(c, r, Tone::Acc, 0.6, 0.6);
            ui.text_plain(c, r, Role::Mono, 0.6, Tone::AccText, &s.to_uppercase());
        }
        let row = self.btns(c, col, 2.6);
        let check = ui.boxed(c, row, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), pad: Some([0.4, 0.6, 0.4, 0.0].into()), ..UiStyle::row() });
        c.entity(check).insert((clickable(), ObAction::Saved));
        let bx = ui.boxed(c, check, UiStyle { w: Some(1.5), h: Some(1.5), align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), ..UiStyle::row() }.outline(Tone::Fg2, 1.0));
        if o.saved {
            c.entity(bx).insert(Pickable::IGNORE);
            ui.boxed(c, bx, UiStyle { w: Some(1.5), h: Some(1.5), ..UiStyle::row() }.bg(Tone::Fg));
            let tick = ui.text(c, bx, Role::DisplayBlack, 1.0, Tone::Ink, "✓");
            c.entity(tick).insert(Node { position_type: PositionType::Absolute, ..default() });
        }
        ui.text(c, check, Role::DisplayBold, 1.0, Tone::Fg, &w.t("key.saved_it"));
        ui.boxed(c, row, UiStyle { w: Some(2.0), ..UiStyle::row() });
        self.button(c, row, Kind::Primary, &w.t("key.continue"), Some("Enter"), ObAction::Continue, State { focus: o.saved, disabled: !o.saved, ..State::NONE });
        // Getting back in.
        let pane = self.abs(c, root, 60.0, 14.6, 31.6);
        let bp = ui.boxed(c, pane, UiStyle { pad: Some([2.0, 2.2].into()), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.75).outline(Tone::AccText, 0.22));
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
        self.anim(c, pane);
        self.keys(c, root, &[("Space", w.t("key.saved_it")), ("Enter", "Continue".into())]);
    }

    fn have(&mut self, c: &mut Commands, root: Entity) {
        let (ui, w, o) = (self.ui, self.w, self.o);
        let col = self.abs(c, root, 8.4, 9.6, 46.0);
        self.h1(c, col, &w.t("have.title"), 3.3);
        let tabs_row = ui.boxed(c, col, UiStyle { margin: Some([1.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        let names = w.list("have.tabs");
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let sel = if o.tab == HaveTab::Link { 0 } else { 1 };
        for (i, t) in ui.tabs_ex(c, tabs_row, &names, sel, true, Some(("Q", "E"))).into_iter().enumerate() {
            c.entity(t).insert((clickable(), ObAction::Tab(if i == 0 { HaveTab::Link } else { HaveTab::Recover })));
        }
        self.anim(c, tabs_row);
        let link = o.tab == HaveTab::Link;
        self.lead(c, col, &w.t(if link { "have.link_lead" } else { "have.recover_lead" }));
        ui.boxed(c, col, UiStyle { h: Some(1.4), ..UiStyle::column() });
        if o.last_hub.is_none() {
            let st = State::focused(o.have_on_hub);
            let hf = self.field(c, col, &w.t("have.hub_label"), Tone::Fg3, None, &o.have_hub, "chimera-hub://...", o.have_on_hub.then(|| Caret::end_of(&o.have_hub)), 42.0, st, None, None);
            c.entity(hf).insert((clickable(), ObAction::HaveField(true)));
            ui.boxed(c, col, UiStyle { h: Some(1.0), ..UiStyle::column() });
        }
        let (label, ph, status) = if link {
            ("Link code".to_string(), "H4K-92Q", Some((Tone::Fg4, w.t("have.link_status"))))
        } else {
            (w.t("have.recover_label"), "XXXX-XXXX-XXXX-XXXX", None)
        };
        let st = State { focus: !o.have_on_hub, error: o.problem.is_some(), ..State::NONE };
        let status_ref = status.as_ref().map(|(t, s)| (*t, s.as_str()));
        let cf = self.field(c, col, &label, Tone::Fg3, None, &o.have_code, ph, (!o.have_on_hub).then(|| Caret::end_of(&o.have_code)), 42.0, st, status_ref, None);
        c.entity(cf).insert((clickable(), ObAction::HaveField(false)));
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
        self.anim(c, note);
        self.keys(c, root, &[("Q / E", "Tab".into()), ("Enter", "Sign in".into()), ("Esc", "Back".into())]);
        let _ = SENTENCE;
    }
}

/// Characters the invite field shows (42 cq at the typed size).
const FIELD_CHARS: usize = 64;

/// What the invite field shows: a link read at Enter with its fingerprint
/// shortened (no caret: Esc clears, typing starts over), else the text's
/// last `FIELD_CHARS` with the caret moved along.
fn field_view(text: &str, caret: Caret, parsed: Option<&InviteInput>) -> (String, Option<Caret>) {
    if let Some(p) = parsed.filter(|p| p.kind == InputKind::Link && text.chars().count() > FIELD_CHARS) {
        let mut out = format!("chimera-hub://{}/", p.host.clone().unwrap_or_default());
        if let Some(f) = &p.fingerprint {
            out.push_str(&format!("{}..{}", &f[..8.min(f.len())], &f[f.len().saturating_sub(8)..]));
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
        return f;
    }
    format!("{} {} .. {} {}", &f[..4], &f[4..8], &f[f.len() - 8..f.len() - 4], &f[f.len() - 4..])
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
        "clipboard" => {
            typed(&mut o, &full);
            o.source = Source::Clipboard;
            o.parsed = InviteInput::parse(&full);
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
        "key" => {
            at_hub(&mut o);
            o.step = Step::Key;
            o.signed_in = Some(staged.file("sam_rocket", None));
            o.recovery = Some(STAGED_KEY.into());
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
                "ui_scene onboarding {other}: welcome, clipboard, checking, used, unknown, unreachable, certificate, hub, name, \
                 name-bad, name-taken, key, have, recover, done, returning, offline or off"
            ));
        }
    }
    *w.resource_mut::<Onboarding>() = o;
    Ok(())
}

/// Fills the welcome field from the clipboard at the first frame when it
/// holds a hub link (nothing is sent until Enter).
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
    }
}
