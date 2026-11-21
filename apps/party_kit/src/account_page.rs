//! Settings > Account (feature `present`; docs/plans/active/onboarding.md,
//! "The account page"): this PC's hub account, opened from the main
//! menu's account chip (`onboarding::show_chip`, `OpenAccount`). A shell
//! screen in Settings' look (the bar's crumb "Main menu ‹ Account", the
//! window, the grey-to-white tabs with badges, one row marker, each row's
//! control in the one control column, the focused row's help in the tile
//! at the right, the action bar), its words in `data/onboarding.toml
//! [account]`.
//!
//! - **Profile**: the username (Enter or a click edits it; its
//!   availability asked of the hub `NAME_PAUSE` after the last key, with
//!   the sign-up's free names when taken, `account::check_rename`; Enter
//!   renames, one a minute, the chip and `account.toml` follow); the
//!   avatar (←→ or a click in the tile's 32 looks: `cl_avatar` at once,
//!   the hub told `AVATAR_PAUSE` after the last change); the hub (its
//!   name, address and whether it answers); **Link another device** (a
//!   10-minute code for "I already have an account" on the other PC,
//!   `account::link_code`); **Sign out** (asks in the row itself: Stay or
//!   Sign out; then this device is revoked at the hub when it answers,
//!   `account.toml` deleted either way, and the first run's Welcome
//!   shows).
//! - **Invites** (an owner's account only, by `/accounts/me`'s role): the
//!   form (intent, recipient, role: Tester or Host), Make invite (the
//!   link shown once with Copy), the open invites (newest first, each
//!   with Revoke), `account::make_invite`, `open_invites`,
//!   `revoke_invite`. The tab's badge counts the open ones.
//! - **Keys**: ↑↓ rows, Enter acts (edits a field, commits it), Esc
//!   leaves a field or the confirm, else closes (the crumb and a right
//!   click too); Q and E the tabs; ←→ the avatar and the role. Nothing
//!   animates in a way that holds a key back.
//! - **Calls** go through `AccountBackend` (`NetworkAccount`: the live
//!   session's ticket, on a thread; `StagedAccount` answers in the same
//!   frame for tests and `ui_scene account <profile|rename|rename-taken|
//!   link|sign-out|invites|invite-made|player>`).
//!
//! Known shortcuts: no display name, device list or new recovery key here
//! yet (hub.md 8); the link code's "valid for" is drawn once, not counted
//! down; the screen rebuilds whole on any change.

use std::sync::{Arc, Mutex};

use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};
use chimera_assets::{
    account::{self, InviteMade, InviteRequest, NameCheck, OpenInvite},
    codes::{self, UsernameRules},
};
use chimera_ui::{
    Role, Tone, UiStyle,
    avatar::{AvatarLook, AvatarMarks, Face},
    controls::{Kind, SENTENCE, State, Ui, clickable},
    form::{BadgeKind, Choice, RowLook},
    motion::UiClock,
    text_edit::{Caret, text_key},
};

use crate::{
    main_menu::MenuUiCamera,
    onboarding::{AccountShown, Onboarding, OnboardingFiles, Shown, Words},
};

/// Over the main menu (Settings' place: never both).
pub const ACCOUNT_Z: i32 = 92;
/// Seconds after the last key before a new username is asked of the hub.
pub const NAME_PAUSE: f64 = crate::onboarding::NAME_PAUSE;
/// Seconds after the last avatar change before the hub is told.
pub const AVATAR_PAUSE: f64 = 0.6;
/// The roles an invite made here may carry (an owner's: the command line).
pub const ROLES: [&str; 2] = ["tester", "host"];
/// The scroll view's key.
const SCROLL_KEY: &str = "account";

/// The account as the hub has it (`/accounts/me`).
pub type Me = account::MeView;

/// The page's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Profile,
    Invites,
}

/// A row of a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Username,
    Avatar,
    Hub,
    Link,
    SignOut,
    Intent,
    Recipient,
    Role,
    Make,
    /// An open invite (its id).
    Open(i64),
}

/// The field taking the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editing {
    Name,
    Intent,
    Recipient,
}

/// A line in the action bar: what the last call did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub error: bool,
    pub text: String,
}

/// The page's state (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AccountPage {
    pub open: bool,
    pub tab: Tab,
    pub focus: usize,
    /// When the page or its tab came up (the rows' wipe).
    pub since: f64,
    pub me: Option<Me>,
    pub editing: Option<Editing>,
    pub name: String,
    pub caret: Caret,
    pub name_check: Option<NameCheck>,
    pub typed_at: f64,
    pub asked_name: Option<String>,
    /// The avatar chosen here, and when (the hub told after a pause).
    pub avatar: Option<u8>,
    pub avatar_at: Option<f64>,
    /// The link code and its minutes.
    pub link: Option<(String, u64)>,
    pub confirm_sign_out: bool,
    pub intent: String,
    pub icaret: Caret,
    pub recipient: String,
    pub rcaret: Caret,
    /// An index into `ROLES`.
    pub role: usize,
    pub made: Option<InviteMade>,
    pub invites: Option<Vec<OpenInvite>>,
    pub said: Option<Said>,
    /// A call under way that the screen waits on (its number).
    pub pending: Option<u32>,
    pub asked: u32,
}

impl AccountPage {
    /// The tabs this account sees (Invites: an owner's).
    pub fn tabs(&self) -> Vec<Tab> {
        if self.me.as_ref().is_some_and(Me::owner) { vec![Tab::Profile, Tab::Invites] } else { vec![Tab::Profile] }
    }

    /// The tab's rows.
    pub fn rows(&self) -> Vec<Row> {
        match self.tab {
            Tab::Profile => vec![Row::Username, Row::Avatar, Row::Hub, Row::Link, Row::SignOut],
            Tab::Invites => {
                let mut v = vec![Row::Intent, Row::Recipient, Row::Role, Row::Make];
                v.extend(self.invites.iter().flatten().map(|i| Row::Open(i.id)));
                v
            }
        }
    }

    pub fn row(&self) -> Option<Row> {
        self.rows().get(self.focus).copied()
    }

    /// The avatar shown: the one chosen here, else the hub's.
    pub fn avatar_now(&self, fallback: u8) -> u8 {
        self.avatar.or(self.me.as_ref().and_then(|m| m.avatar)).unwrap_or(fallback)
    }

    /// Whether the typed name can be had (the hub said so for it, or it
    /// is the account's own).
    pub fn name_ok(&self) -> bool {
        let folded = self.name.trim().to_ascii_lowercase();
        self.name_check.as_ref().is_some_and(|n| n.available && n.username == folded)
    }

    fn go_tab(&mut self, tab: Tab, now: f64) {
        if self.tab != tab {
            self.tab = tab;
            self.focus = 0;
            self.since = now;
            self.editing = None;
            self.confirm_sign_out = false;
        }
    }
}

/// Whether the page has the keys (the menus behind it take none).
pub fn takes_keys(p: Option<&AccountPage>) -> bool {
    p.is_some_and(|p| p.open)
}

// ---------------------------------------------------------------------------
// The hub.

/// What the page asks of the hub (the network's, or a staged one).
pub trait AccountHub: Send + Sync {
    fn me(&self) -> Result<Me, String>;
    fn check_rename(&self, name: &str) -> Result<NameCheck, String>;
    fn rename(&self, name: &str) -> Result<Me, String>;
    fn set_avatar(&self, avatar: u8) -> Result<Me, String>;
    /// A link code and its minutes.
    fn link_code(&self) -> Result<(String, u64), String>;
    fn sign_out(&self) -> Result<(), String>;
    fn make_invite(&self, request: &InviteRequest) -> Result<InviteMade, String>;
    fn open_invites(&self) -> Result<Vec<OpenInvite>, String>;
    fn revoke_invite(&self, id: i64) -> Result<(), String>;
}

/// The live session's hub (`chimera_assets::account`).
pub struct NetworkAccount;

fn session() -> Result<(account::AccountFile, String), String> {
    let s = account::live().ok_or("this PC isn't signed in")?;
    let t = s.ticket()?;
    Ok((s.file(), t))
}


impl AccountHub for NetworkAccount {
    fn me(&self) -> Result<Me, String> {
        let (f, t) = session()?;
        account::view(account::me(&f, &t)?)
    }
    fn check_rename(&self, name: &str) -> Result<NameCheck, String> {
        let (f, t) = session()?;
        account::check_rename(&f, &t, name)
    }
    fn rename(&self, name: &str) -> Result<Me, String> {
        let (f, t) = session()?;
        account::view(account::rename(&f, &t, name)?)
    }
    fn set_avatar(&self, avatar: u8) -> Result<Me, String> {
        let (f, t) = session()?;
        account::view(account::set_avatar(&f, &t, avatar)?)
    }
    fn link_code(&self) -> Result<(String, u64), String> {
        let (f, t) = session()?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        account::link_code(&f, &t).map(|l| (l.code, l.expires.saturating_sub(now).div_ceil(60)))
    }
    fn sign_out(&self) -> Result<(), String> {
        let (f, t) = session()?;
        account::sign_out(&f, &t)
    }
    fn make_invite(&self, request: &InviteRequest) -> Result<InviteMade, String> {
        let (f, t) = session()?;
        account::make_invite(&f, &t, request)
    }
    fn open_invites(&self) -> Result<Vec<OpenInvite>, String> {
        let (f, t) = session()?;
        account::open_invites(&f, &t)
    }
    fn revoke_invite(&self, id: i64) -> Result<(), String> {
        let (f, t) = session()?;
        account::revoke_invite(&f, &t, id)
    }
}

/// A made-up hub (`ui_scene account`, tests): Sam Rocket, owner of
/// Friday Night Hub; `rocket`, `sam` and `player` are taken; three open
/// invites. `owner: false` is a player's account.
pub struct StagedAccount {
    state: Mutex<(Me, Vec<OpenInvite>, i64)>,
    taken: Vec<String>,
}

/// The staged invite's code (made up).
pub const STAGED_INVITE: &str = "K7Q-M2P-9TR";
/// The staged link code (made up).
pub const STAGED_LINK: &str = "H4K-9QX";
/// When the staged invites were made (2026-11-21, unix).
const STAGED_NOW: u64 = 1_795_000_000;

impl StagedAccount {
    pub fn new(owner: bool) -> Self {
        let me = Me {
            username: "sam_rocket".into(),
            display_name: "Sam Rocket".into(),
            avatar: Some(13),
            role: if owner { "owner" } else { "tester" }.into(),
            previous_usernames: vec!["sam_r".into()],
        };
        let open = |id: i64, name: &str, intent: &str, role: &str, days: u64| OpenInvite {
            id,
            name: name.into(),
            intent: Some(intent.into()),
            recipient: Some(name.into()),
            role: role.into(),
            created: STAGED_NOW - 86_400 * (14 - days),
            expires: STAGED_NOW + 86_400 * days,
        };
        let invites = vec![
            open(3, "Kim", "sent on Discord", "tester", 13),
            open(2, "Lee", "the LAN box upstairs", "host", 9),
            open(1, "Robin", "Friday regulars", "tester", 4),
        ];
        StagedAccount { state: Mutex::new((me, invites, 4)), taken: ["rocket", "sam", "player"].map(String::from).to_vec() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (Me, Vec<OpenInvite>, i64)> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn owner(&self) -> Result<(), String> {
        if self.lock().0.owner() { Ok(()) } else { Err("the hub refused: only the hub's owner manages invites (403)".into()) }
    }
}

impl AccountHub for StagedAccount {
    fn me(&self) -> Result<Me, String> {
        Ok(self.lock().0.clone())
    }
    fn check_rename(&self, name: &str) -> Result<NameCheck, String> {
        let folded = name.trim().to_ascii_lowercase();
        let own = self.lock().0.username.clone();
        let taken = |x: &str| x != own && self.taken.iter().any(|t| t == x);
        Ok(match codes::choose_username(&folded) {
            Err(p) => NameCheck { username: folded, available: false, problem: Some(p), suggestions: vec![] },
            Ok(u) if taken(&u) => NameCheck { suggestions: codes::suggestions(&u, 3, taken), username: u, available: false, problem: Some("taken".into()) },
            Ok(u) => NameCheck { username: u, available: true, problem: None, suggestions: vec![] },
        })
    }
    fn rename(&self, name: &str) -> Result<Me, String> {
        let c = self.check_rename(name)?;
        if !c.available {
            return Err(format!("the hub refused: the username {:?} is taken (409)", c.username));
        }
        let mut s = self.lock();
        let old = std::mem::replace(&mut s.0.username, c.username);
        s.0.previous_usernames.insert(0, old);
        Ok(s.0.clone())
    }
    fn set_avatar(&self, avatar: u8) -> Result<Me, String> {
        let mut s = self.lock();
        s.0.avatar = Some(avatar);
        Ok(s.0.clone())
    }
    fn link_code(&self) -> Result<(String, u64), String> {
        Ok((STAGED_LINK.into(), 10))
    }
    fn sign_out(&self) -> Result<(), String> {
        Ok(())
    }
    fn make_invite(&self, request: &InviteRequest) -> Result<InviteMade, String> {
        self.owner()?;
        if request.intent.trim().is_empty() {
            return Err("the hub refused: an invite needs an intent: why it was made (400)".into());
        }
        let mut s = self.lock();
        let id = s.2;
        s.2 += 1;
        let name = request.recipient.clone().filter(|r| !r.trim().is_empty()).unwrap_or_else(|| request.intent.clone());
        s.1.insert(0, OpenInvite {
            id,
            name: name.clone(),
            intent: Some(request.intent.clone()),
            recipient: request.recipient.clone(),
            role: request.role.clone().unwrap_or_else(|| "tester".into()),
            created: STAGED_NOW,
            expires: STAGED_NOW + 14 * 86_400,
        });
        let fp = "3f9a0b71c2d45e6f".repeat(4);
        Ok(InviteMade {
            name,
            code: STAGED_INVITE.into(),
            expires: STAGED_NOW + 14 * 86_400,
            link: Some(format!("{}hub.example.net:27080/{fp}?code={STAGED_INVITE}", account::SCHEME)),
        })
    }
    fn open_invites(&self) -> Result<Vec<OpenInvite>, String> {
        self.owner()?;
        Ok(self.lock().1.clone())
    }
    fn revoke_invite(&self, id: i64) -> Result<(), String> {
        self.owner()?;
        let mut s = self.lock();
        let before = s.1.len();
        s.1.retain(|i| i.id != id);
        if s.1.len() == before { Err("the hub refused: no open invite with that id (404)".into()) } else { Ok(()) }
    }
}

/// The hub the page asks; `inline` answers in the same frame (tests,
/// staged scenes: nothing of this PC's account is touched), else on a
/// thread.
#[derive(Resource, Clone)]
pub struct AccountBackend {
    pub hub: Arc<dyn AccountHub>,
    pub inline: bool,
}

impl Default for AccountBackend {
    fn default() -> Self {
        AccountBackend { hub: Arc::new(NetworkAccount), inline: false }
    }
}

impl AccountBackend {
    pub fn staged(owner: bool) -> Self {
        AccountBackend { hub: Arc::new(StagedAccount::new(owner)), inline: true }
    }
}

#[derive(Clone, Debug)]
enum Ask {
    Me,
    Check(String),
    Rename(String),
    Avatar(u8),
    Link,
    SignOut,
    Make(InviteRequest),
    Invites,
    Revoke(i64),
}

#[derive(Clone, Debug)]
enum Reply {
    Me(Result<Me, String>),
    Checked(String, Result<NameCheck, String>),
    Renamed(Result<Me, String>),
    Avatar(Result<Me, String>),
    Link(Result<(String, u64), String>),
    SignedOut(Result<(), String>),
    Made(Result<InviteMade, String>),
    Invites(Result<Vec<OpenInvite>, String>),
    Revoked(i64, Result<(), String>),
}

#[derive(Resource, Clone, Default)]
struct Answers(Arc<Mutex<Vec<(u32, Reply)>>>);

fn ask(p: &mut AccountPage, backend: &AccountBackend, answers: &Answers, what: Ask) {
    p.asked += 1;
    let n = p.asked;
    if !matches!(what, Ask::Me | Ask::Check(_) | Ask::Avatar(_) | Ask::Invites) {
        p.pending = Some(n);
    }
    let hub = backend.hub.clone();
    let out = answers.0.clone();
    let run = move || {
        let reply = match what {
            Ask::Me => Reply::Me(hub.me()),
            Ask::Check(name) => {
                let r = hub.check_rename(&name);
                Reply::Checked(name, r)
            }
            Ask::Rename(name) => Reply::Renamed(hub.rename(&name)),
            Ask::Avatar(a) => Reply::Avatar(hub.set_avatar(a)),
            Ask::Link => Reply::Link(hub.link_code()),
            Ask::SignOut => Reply::SignedOut(hub.sign_out()),
            Ask::Make(r) => Reply::Made(hub.make_invite(&r)),
            Ask::Invites => Reply::Invites(hub.open_invites()),
            Ask::Revoke(id) => Reply::Revoked(id, hub.revoke_invite(id)),
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

/// What a click (or a key) does.
#[derive(Component, Message, Clone, Debug, PartialEq)]
pub enum AccountAction {
    Tab(Tab),
    /// A row (the pointer pressing it): the focus.
    Row(usize),
    /// The focused row's action (Enter).
    Activate,
    Edit(Editing),
    Suggest(String),
    Avatar(u8),
    Link,
    SignOut,
    SignOutConfirm,
    SignOutStay,
    Role(usize),
    Make,
    CopyLink,
    Revoke(i64),
    Close,
}

/// The main menu's account chip: a click opens the page.
#[derive(Component, Clone, Copy, Debug)]
pub struct OpenAccount;

#[derive(Component)]
pub struct AccountRoot;

pub struct AccountPagePlugin;

impl Plugin for AccountPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AccountPage>()
            .init_resource::<AccountBackend>()
            .init_resource::<Answers>()
            .init_resource::<Words>()
            .init_resource::<OnboardingFiles>()
            .add_message::<AccountAction>();
        if !app.world().contains_resource::<Messages<KeyboardInput>>() {
            app.add_message::<KeyboardInput>();
        }
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "account", scene);
        let _ = chimera_core::console::ConsoleAppExt::console_command(app, "account_page", "account_page [0|1]: open this PC's account page (Settings > Account), or close it.", |w, a| {
            let on = a.first().is_none_or(|v| v != "0");
            if on {
                open_page(w);
            } else {
                w.resource_mut::<AccountPage>().open = false;
            }
            Ok(None)
        });
        app.add_systems(Update, (open_from_chip, clicks, keys, poll, pauses, act, show).chain());
    }
}

/// Open the page (the chip's click, `account_page 1`): the account asked
/// of the hub again, its invites too for an owner.
pub fn open_page(w: &mut World) {
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let backend = w.resource::<AccountBackend>().clone();
    let answers = w.resource::<Answers>().clone();
    let mut p = w.resource_mut::<AccountPage>();
    let (me, invites, asked) = (p.me.clone(), p.invites.clone(), p.asked);
    *p = AccountPage { open: true, since: now, me, invites, asked, ..default() };
    ask(&mut p, &backend, &answers, Ask::Me);
    if p.me.as_ref().is_some_and(Me::owner) {
        ask(&mut p, &backend, &answers, Ask::Invites);
    }
}

fn open_from_chip(pressed: Query<&Interaction, (Changed<Interaction>, With<OpenAccount>)>, mut commands: Commands) {
    if pressed.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(open_page);
    }
}

fn clicks(pressed: Query<(&Interaction, &AccountAction), Changed<Interaction>>, p: Res<AccountPage>, mut out: MessageWriter<AccountAction>) {
    if !p.open {
        return;
    }
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(a.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn keys(
    mut events: MessageReader<KeyboardInput>,
    buttons: Option<Res<ButtonInput<KeyCode>>>,
    mut p: ResMut<AccountPage>,
    onboarding: Option<Res<Onboarding>>,
    mut out: MessageWriter<AccountAction>,
    clock: Res<UiClock>,
    guard: Option<ResMut<chimera_ui::shell::BackGuard>>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    (shared, mut own): (Option<ResMut<chimera_ui::clipboard::Clipboard>>, Local<chimera_ui::clipboard::Clipboard>),
) {
    if !p.open || onboarding.is_some_and(|o| o.active) || console.is_some_and(|c| c.open) {
        events.clear();
        return;
    }
    if p.editing.is_some()
        && let Some(mut g) = guard
    {
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
        let rows = p.rows();
        match &k.logical_key {
            Key::Escape => {
                if p.editing.take().is_some() {
                    p.name_check = None;
                } else if p.confirm_sign_out {
                    p.confirm_sign_out = false;
                } else {
                    out.write(AccountAction::Close);
                }
                continue;
            }
            Key::Enter => {
                match p.editing {
                    Some(Editing::Name) => {
                        if p.name.trim().eq_ignore_ascii_case(p.me.as_ref().map_or("", |m| m.username.as_str())) {
                            p.editing = None;
                        } else {
                            out.write(AccountAction::Activate);
                        }
                    }
                    // A form's field: on to the next row.
                    Some(_) => {
                        p.editing = None;
                        p.focus = (p.focus + 1).min(rows.len().saturating_sub(1));
                    }
                    None => {
                        out.write(AccountAction::Activate);
                    }
                }
                continue;
            }
            Key::Tab if p.editing.is_some() => {
                p.editing = None;
                p.focus = if shift { p.focus.saturating_sub(1) } else { (p.focus + 1).min(rows.len().saturating_sub(1)) };
                continue;
            }
            Key::ArrowUp | Key::ArrowDown if p.editing.is_none() => {
                let n = rows.len().max(1);
                p.focus = if k.logical_key == Key::ArrowUp { (p.focus + n - 1) % n } else { (p.focus + 1) % n };
                p.confirm_sign_out = false;
                continue;
            }
            Key::ArrowLeft | Key::ArrowRight if p.editing.is_none() => {
                let d: i16 = if k.logical_key == Key::ArrowLeft { -1 } else { 1 };
                match p.row() {
                    Some(Row::Avatar) => {
                        let now_a = p.avatar_now(0) as i16;
                        out.write(AccountAction::Avatar((now_a + d).rem_euclid(32) as u8));
                    }
                    Some(Row::Role) => {
                        out.write(AccountAction::Role(if d < 0 { 0 } else { 1 }));
                    }
                    _ => {}
                }
                continue;
            }
            Key::Character(c) if p.editing.is_none() && !ctrl && (c.eq_ignore_ascii_case("q") || c.eq_ignore_ascii_case("e")) => {
                let tabs = p.tabs();
                let at = tabs.iter().position(|t| *t == p.tab).unwrap_or(0);
                let to = if c.eq_ignore_ascii_case("q") { at.saturating_sub(1) } else { (at + 1).min(tabs.len() - 1) };
                out.write(AccountAction::Tab(tabs[to]));
                continue;
            }
            Key::Character(c) if p.editing.is_none() && ctrl && c.eq_ignore_ascii_case("c") && p.made.is_some() => {
                out.write(AccountAction::CopyLink);
                continue;
            }
            _ => {}
        }
        let Some(edit) = p.editing else { continue };
        let Some(tk) = text_key(&k.logical_key, k.key_code, ctrl, shift) else { continue };
        let p = &mut *p;
        match edit {
            Editing::Name => {
                let mut caret = p.caret;
                if caret.key(&mut p.name, tk, clipboard, codes::NAME_MAX, false) {
                    p.typed_at = now;
                    p.said = None;
                }
                p.caret = caret;
            }
            Editing::Intent => {
                let mut caret = p.icaret;
                caret.key(&mut p.intent, tk, clipboard, 200, false);
                p.icaret = caret;
            }
            Editing::Recipient => {
                let mut caret = p.rcaret;
                caret.key(&mut p.recipient, tk, clipboard, 200, false);
                p.rcaret = caret;
            }
        }
    }
}

/// The name asked of the hub after a pause; the avatar told after one.
fn pauses(mut p: ResMut<AccountPage>, backend: Res<AccountBackend>, answers: Res<Answers>, clock: Res<UiClock>) {
    if !p.open {
        return;
    }
    let now = clock.now;
    if p.editing == Some(Editing::Name) && now - p.typed_at >= NAME_PAUSE {
        let folded = p.name.trim().to_ascii_lowercase();
        if !folded.is_empty() && p.asked_name.as_deref() != Some(folded.as_str()) && UsernameRules::of(&folded).all() {
            p.asked_name = Some(folded.clone());
            ask(&mut p, &backend, &answers, Ask::Check(folded));
        }
    }
    if let (Some(at), Some(a)) = (p.avatar_at, p.avatar)
        && now - at >= AVATAR_PAUSE
    {
        p.avatar_at = None;
        ask(&mut p, &backend, &answers, Ask::Avatar(a));
    }
}

/// The hub's answers.
fn poll(answers: Res<Answers>, mut p: ResMut<AccountPage>, backend: Res<AccountBackend>, words: Res<Words>, mut commands: Commands) {
    let got: Vec<(u32, Reply)> = std::mem::take(&mut *answers.0.lock().unwrap_or_else(|e| e.into_inner()));
    for (n, reply) in got {
        if p.pending == Some(n) {
            p.pending = None;
        }
        let err = |e: &str| Said { error: true, text: said_error(e) };
        match reply {
            Reply::Me(Ok(me)) => {
                let owner = me.owner() && p.invites.is_none();
                keep_names(&mut commands, &me, backend.inline);
                p.me = Some(me);
                if owner {
                    ask(&mut p, &backend, &answers, Ask::Invites);
                }
            }
            Reply::Me(Err(e)) => p.said = Some(err(&e)),
            Reply::Checked(name, Ok(c)) => {
                if p.name.trim().eq_ignore_ascii_case(&name) {
                    p.name_check = Some(c);
                }
            }
            Reply::Checked(_, Err(e)) => p.said = Some(err(&e)),
            Reply::Renamed(Ok(me)) => {
                keep_names(&mut commands, &me, backend.inline);
                p.said = Some(Said { error: false, text: words.f("account.renamed", &[("user", &me.username)]) });
                p.me = Some(me);
                p.editing = None;
                p.name_check = None;
            }
            Reply::Avatar(Ok(me)) => p.me = Some(me),
            Reply::Renamed(Err(e)) | Reply::Avatar(Err(e)) | Reply::Link(Err(e)) | Reply::Made(Err(e)) | Reply::Invites(Err(e)) => p.said = Some(err(&e)),
            Reply::Link(Ok(l)) => p.link = Some(l),
            Reply::SignedOut(r) => {
                // Signed out here either way: a hub that can't be reached
                // leaves the device for the owner to revoke.
                let note = r.err().map(|_| words.t("account.signed_out_offline"));
                let inline = backend.inline;
                commands.queue(move |w: &mut World| signed_out(w, note, inline));
            }
            Reply::Made(Ok(m)) => {
                p.said = Some(Said { error: false, text: words.f("account.made", &[("name", &m.name)]) });
                p.made = Some(m);
                p.intent.clear();
                p.recipient.clear();
                p.icaret = Caret::default();
                p.rcaret = Caret::default();
                ask(&mut p, &backend, &answers, Ask::Invites);
            }
            Reply::Invites(Ok(list)) => {
                p.invites = Some(list);
                let n = p.rows().len();
                if p.focus >= n {
                    p.focus = n.saturating_sub(1);
                }
            }
            Reply::Revoked(id, r) => match r {
                Ok(()) => {
                    let name = p.invites.iter().flatten().find(|i| i.id == id).map(|i| i.name.clone()).unwrap_or_default();
                    p.said = Some(Said { error: false, text: words.f("account.revoked", &[("name", &name)]) });
                    if let Some(l) = p.invites.as_mut() {
                        l.retain(|i| i.id != id);
                    }
                    let n = p.rows().len();
                    p.focus = p.focus.min(n.saturating_sub(1));
                }
                Err(e) => p.said = Some(err(&e)),
            },
        }
    }
}

/// A call's error in a player's words.
pub fn said_error(e: &str) -> String {
    match account::Failure::of(e) {
        account::Failure::Unreachable(_) => "Can't reach the hub right now: try again in a moment.".into(),
        account::Failure::Certificate { .. } => "The hub's certificate isn't the one this PC knows: ask its owner.".into(),
        account::Failure::Refused { message, .. } | account::Failure::Other(message) => {
            let mut c = message.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect::<String>() + ".")
        }
    }
}

/// The account's names as the hub has them, into the chip and (a live
/// session, not a staged one) `account.toml`.
fn keep_names(commands: &mut Commands, me: &Me, staged: bool) {
    let me = me.clone();
    commands.queue(move |w: &mut World| {
        if let Some(mut shown) = w.get_resource_mut::<AccountShown>()
            && let Some(s) = shown.0.as_mut()
            && (s.username != me.username || (!me.display_name.is_empty() && s.display != me.display_name))
        {
            s.username = me.username.clone();
            if !me.display_name.is_empty() {
                s.display = me.display_name.clone();
            }
        }
        if staged {
            return;
        }
        if let Some(s) = account::live()
            && s.file().username != me.username
        {
            let mut f = s.file();
            f.username = me.username.clone();
            if !me.display_name.is_empty() {
                f.display_name = me.display_name.clone();
            }
            let path = s.path().map(std::path::PathBuf::from);
            if let Some(p) = &path {
                let _ = f.save_to(p);
            }
            account::set_live(Some(Arc::new(account::Session::new(f, path))));
        }
    });
}

/// After Sign out: `account.toml` deleted, the chip gone, the page shut,
/// and the first run's Welcome up (no intro). `staged`: this PC's live
/// session untouched.
fn signed_out(w: &mut World, note: Option<String>, staged: bool) {
    let files = w.get_resource::<OnboardingFiles>().cloned().unwrap_or_default();
    if let Some(path) = &files.account {
        let _ = std::fs::remove_file(path);
    }
    if !staged {
        account::set_live(None);
        w.remove_resource::<chimera_net::net::hub::Hub>();
    }
    w.insert_resource(AccountShown(None));
    *w.resource_mut::<AccountPage>() = AccountPage::default();
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    if let Some(mut o) = w.get_resource_mut::<Onboarding>() {
        *o = Onboarding { active: true, since: now, said: note, ..default() };
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    mut actions: MessageReader<AccountAction>,
    mut p: ResMut<AccountPage>,
    backend: Res<AccountBackend>,
    answers: Res<Answers>,
    clock: Res<UiClock>,
    words: Res<Words>,
    mut console: Option<ResMut<chimera_core::console::Console>>,
    mut clipboard: Option<ResMut<chimera_ui::clipboard::Clipboard>>,
) {
    let now = clock.now;
    for a in actions.read() {
        if !p.open {
            continue;
        }
        let rows = p.rows();
        match a.clone() {
            AccountAction::Close => {
                p.open = false;
                p.editing = None;
                p.confirm_sign_out = false;
            }
            AccountAction::Tab(t) => p.go_tab(t, now),
            AccountAction::Row(i) => {
                let was = p.focus;
                p.focus = i.min(rows.len().saturating_sub(1));
                if was != p.focus {
                    p.editing = None;
                    p.confirm_sign_out = false;
                }
            }
            AccountAction::Activate => {
                match rows.get(p.focus) {
                    Some(Row::Username) => match p.editing {
                        Some(Editing::Name) if p.name_ok() && p.pending.is_none() => {
                            let name = p.name.trim().to_ascii_lowercase();
                            ask(&mut p, &backend, &answers, Ask::Rename(name));
                        }
                        Some(Editing::Name) => {}
                        _ => start_name(&mut p, now),
                    },
                    Some(Row::Intent) => edit(&mut p, Editing::Intent),
                    Some(Row::Recipient) => edit(&mut p, Editing::Recipient),
                    Some(Row::Link) if p.pending.is_none() => ask(&mut p, &backend, &answers, Ask::Link),
                    Some(Row::SignOut) if p.confirm_sign_out && p.pending.is_none() => ask(&mut p, &backend, &answers, Ask::SignOut),
                    Some(Row::SignOut) => p.confirm_sign_out = true,
                    Some(Row::Role) => p.role = (p.role + 1) % ROLES.len(),
                    Some(Row::Make) => make(&mut p, &backend, &answers, &words),
                    Some(Row::Open(id)) if p.pending.is_none() => {
                        let id = *id;
                        ask(&mut p, &backend, &answers, Ask::Revoke(id));
                    }
                    _ => {}
                }
            }
            AccountAction::Edit(Editing::Name) => {
                p.focus = 0;
                start_name(&mut p, now);
            }
            AccountAction::Edit(e) => {
                p.focus = rows.iter().position(|r| matches!((r, e), (Row::Intent, Editing::Intent) | (Row::Recipient, Editing::Recipient))).unwrap_or(p.focus);
                edit(&mut p, e);
            }
            AccountAction::Suggest(s) => {
                p.name = s.clone();
                p.caret = Caret::end_of(&s);
                p.editing = Some(Editing::Name);
                p.typed_at = now - NAME_PAUSE;
            }
            AccountAction::Avatar(b) => {
                p.avatar = Some(b);
                p.avatar_at = Some(now);
                p.focus = rows.iter().position(|r| *r == Row::Avatar).unwrap_or(p.focus);
                if let Some(c) = console.as_deref_mut() {
                    c.submit(format!("cl_avatar {b}"));
                }
            }
            AccountAction::Link => {
                if p.pending.is_none() {
                    ask(&mut p, &backend, &answers, Ask::Link);
                }
            }
            AccountAction::SignOut => {
                p.focus = rows.iter().position(|r| *r == Row::SignOut).unwrap_or(p.focus);
                p.confirm_sign_out = true;
            }
            AccountAction::SignOutStay => p.confirm_sign_out = false,
            AccountAction::SignOutConfirm => {
                if p.confirm_sign_out && p.pending.is_none() {
                    ask(&mut p, &backend, &answers, Ask::SignOut);
                }
            }
            AccountAction::Role(i) => p.role = i.min(ROLES.len() - 1),
            AccountAction::Make => make(&mut p, &backend, &answers, &words),
            AccountAction::CopyLink => {
                if let (Some(m), Some(c)) = (p.made.as_ref(), clipboard.as_deref_mut()) {
                    c.set(m.link.as_deref().unwrap_or(&m.code));
                    p.said = Some(Said { error: false, text: words.t("account.copied") });
                }
            }
            AccountAction::Revoke(id) => {
                if p.pending.is_none() {
                    ask(&mut p, &backend, &answers, Ask::Revoke(id));
                }
            }
        }
    }
}

fn start_name(p: &mut AccountPage, now: f64) {
    let current = p.me.as_ref().map(|m| m.username.clone()).unwrap_or_default();
    p.name = current.clone();
    p.caret = Caret::end_of(&current);
    p.editing = Some(Editing::Name);
    p.name_check = Some(NameCheck { username: current.clone(), available: true, problem: None, suggestions: vec![] });
    p.asked_name = Some(current);
    p.typed_at = now;
    p.confirm_sign_out = false;
}

fn edit(p: &mut AccountPage, e: Editing) {
    if p.editing == Some(e) {
        p.editing = None;
        return;
    }
    p.editing = Some(e);
    match e {
        Editing::Intent => p.icaret = Caret::end_of(&p.intent),
        Editing::Recipient => p.rcaret = Caret::end_of(&p.recipient),
        Editing::Name => {}
    }
}

fn make(p: &mut AccountPage, backend: &AccountBackend, answers: &Answers, words: &Words) {
    if p.pending.is_some() {
        return;
    }
    if p.intent.trim().is_empty() {
        p.said = Some(Said { error: true, text: words.t("account.intent_needed") });
        p.focus = 0;
        p.editing = Some(Editing::Intent);
        return;
    }
    p.editing = None;
    let request = InviteRequest {
        intent: p.intent.trim().to_string(),
        recipient: Some(p.recipient.trim().to_string()).filter(|r| !r.is_empty()),
        role: Some(ROLES[p.role.min(ROLES.len() - 1)].to_string()),
    };
    ask(p, backend, answers, Ask::Make(request));
}

// ---------------------------------------------------------------------------
// Drawing.

/// "5 Dec" for unix seconds (UTC).
pub fn day_month(secs: u64) -> String {
    // Days to the civil date (Howard Hinnant's algorithm, public domain).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{d} {}", MONTHS[(m - 1) as usize])
}

/// A long link shortened in its middle (the fingerprint's first and last
/// four), so it fits a line.
pub fn short_link(link: &str) -> String {
    let Some(rest) = link.strip_prefix(account::SCHEME) else { return link.to_string() };
    let (head, query) = rest.split_once('?').map_or((rest, ""), |(h, q)| (h, q));
    let (host, fp) = head.split_once('/').unwrap_or((head, ""));
    let fp = if fp.chars().count() > 12 { format!("{}..{}", &fp[..4], &fp[fp.len() - 4..]) } else { fp.to_string() };
    let q = if query.is_empty() { String::new() } else { format!("?{query}") };
    format!("{}{host}/{fp}{q}", account::SCHEME)
}

type DrawKey = (AccountPage, Option<Shown>, (String, bool), i32, (u32, u32), Option<Entity>, chimera_ui::shell::Party);

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    p: Res<AccountPage>,
    words: Res<Words>,
    shown_account: Res<AccountShown>,
    hub: Option<Res<chimera_net::net::hub::Hub>>,
    avatar: Option<Res<chimera_client::client::profile::AvatarSetting>>,
    party: Option<Res<chimera_ui::shell::Party>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<AccountRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    (mut images, mut icons): (Option<ResMut<Assets<Image>>>, Option<ResMut<chimera_ui::icons::IconImages>>),
    mut drawn: Local<Option<DrawKey>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let cam = camera.iter().next();
    let key = p.open.then(|| {
        let mut snap = p.clone();
        snap.typed_at = 0.0;
        snap.asked_name = None;
        snap.avatar_at = None;
        let hub_state = match hub.as_deref() {
            Some(h) => (if h.doc.is_some() { h.name() } else { String::new() }, h.offline()),
            None => (String::new(), false),
        };
        (
            snap,
            shown_account.0.clone(),
            hub_state,
            avatar.as_deref().map_or(-1, |s| s.0),
            ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32),
            cam,
            party.as_deref().cloned().unwrap_or_default(),
        )
    });
    if *drawn == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((_, shown, (hub_name, offline), look, _, cam, party)) = key.clone() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let hub_name = if hub_name.is_empty() { shown.as_ref().map(|s| s.hub.clone()).unwrap_or_default() } else { hub_name };
        let host = account::live().map(|s| account::hub_link_of(&s.file()).host).unwrap_or_else(|| "hub.example.net:27080".into());
        let fallback = u8::try_from(look).unwrap_or(0);
        let mut d = Draw {
            ui: &ui,
            w: &words,
            p: &p,
            shown: shown.as_ref(),
            hub: &hub_name,
            host: &host,
            offline,
            fallback,
            images: images.as_deref_mut(),
            icons: icons.as_deref_mut(),
        };
        let r = d.screen(&mut commands, &party);
        if let Some(cam) = cam {
            commands.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *drawn = key;
}

struct Draw<'a, 'b> {
    ui: &'a Ui<'a>,
    w: &'a Words,
    p: &'a AccountPage,
    shown: Option<&'a Shown>,
    hub: &'a str,
    host: &'a str,
    offline: bool,
    fallback: u8,
    images: Option<&'b mut Assets<Image>>,
    icons: Option<&'b mut chimera_ui::icons::IconImages>,
}

impl Draw<'_, '_> {
    fn t(&self, key: &str) -> String {
        self.w.t(&format!("account.{key}"))
    }

    fn username(&self) -> String {
        self.p.me.as_ref().map(|m| m.username.clone()).or(self.shown.map(|s| s.username.clone())).unwrap_or_default()
    }

    fn display(&self) -> String {
        self.p
            .me
            .as_ref()
            .map(|m| m.display_name.clone())
            .filter(|d| !d.is_empty())
            .or(self.shown.map(|s| s.display.clone()))
            .unwrap_or_else(|| self.username())
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

    fn screen(&mut self, c: &mut Commands, party: &chimera_ui::shell::Party) -> Entity {
        let ui = self.ui;
        let t = ui.theme;
        let p = self.p;
        let root = c
            .spawn((
                AccountRoot,
                chimera_ui::shell::ShellScreen,
                Name::new("account page"),
                GlobalZIndex(ACCOUNT_Z),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(t.tone(Tone::Ink)),
                bevy::ui::FocusPolicy::Block,
            ))
            .id();
        // The ground: Settings' navy fall from the top left.
        let wash = ui.boxed(c, root, UiStyle { w_pct: Some(100.0), h_pct: Some(100.0), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, None]));
        c.entity(wash).insert(BackgroundGradient(vec![
            LinearGradient::new(
                162f32.to_radians(),
                vec![ColorStop::percent(t.tone_a(Tone::Hero, 0.14), 0.0), ColorStop::percent(t.tone_a(Tone::Hero, 0.0), 60.0)],
            )
            .into(),
        ]));
        let frame = ui.boxed(c, root, UiStyle { w: Some(100.0), h: Some(56.25), shrink: Some(0.0), ..UiStyle::row() });
        let sh = chimera_ui::shell::ShellLayout::of(t);
        let title = self.t("title");
        if let (Some(images), Some(icons)) = (self.images.as_deref_mut(), self.icons.as_deref_mut()) {
            let bar = chimera_ui::shell::Bar {
                left: chimera_ui::shell::Left::Crumb { parent: "Main menu", title: &title },
                party: &party.members,
                party_max: party.max,
                invite: party.invite,
                open: Some(chimera_ui::shell::ShellTile::Profile),
            };
            chimera_ui::shell::bar(ui, c, frame, images, icons, &bar);
        }
        let win = chimera_ui::shell::window(ui, c, frame);
        // The tabs: grey to white, Invites' badge its open count.
        let tabs = p.tabs();
        let names: Vec<String> = tabs.iter().map(|t| self.t(match t { Tab::Profile => "tab_profile", Tab::Invites => "tab_invites" })).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let badges: Vec<Option<String>> = tabs
            .iter()
            .map(|t| (*t == Tab::Invites).then(|| p.invites.as_ref().map(|l| l.len())).flatten().filter(|n| *n > 0).map(|n| n.to_string()))
            .collect();
        let tabbar = ui.boxed(c, win, UiStyle::column().abs([Some(sh.rows_in), Some(1.6), Some(sh.side_w + 2.0 * sh.rows_in), None]));
        let at = tabs.iter().position(|x| *x == p.tab).unwrap_or(0);
        for (i, e) in ui.tabs_badged(c, tabbar, &refs, &badges, at, false, (refs.len() > 1).then_some(("Q", "E"))).into_iter().enumerate() {
            c.entity(e).insert(AccountAction::Tab(tabs[i]));
        }
        let cols = ui.boxed(c, win, UiStyle { gap: Some(2.0 * sh.rows_in), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(sh.rows_in), Some(4.6), Some(sh.rows_in), Some(sh.act_h + 1.4)]));
        let sc = ui.scroll(c, cols, SCROLL_KEY, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() }, UiStyle::column());
        let list = sc.content;
        let rows = p.rows();
        let mut open_drawn = false;
        for (i, r) in rows.iter().enumerate() {
            if matches!(r, Row::Open(_)) && !open_drawn {
                open_drawn = true;
                let n = p.invites.as_ref().map_or(0, |l| l.len()).to_string();
                ui.section(c, list, &self.t("open_section"), Some(&n));
            }
            self.row(c, list, i, *r);
        }
        if p.tab == Tab::Invites && p.invites.as_ref().is_some_and(|l| l.is_empty()) {
            ui.section(c, list, &self.t("open_section"), Some("0"));
            ui.sentence(c, list, SENTENCE, Tone::Fg3, &self.t("open_none"));
        }
        self.help(c, cols, sh.side_w);
        // The action bar: what the last call did, else the page's line.
        let bar = chimera_ui::shell::action_bar(ui, c, win);
        let st = ui.boxed(c, bar, UiStyle { gap: Some(0.6), grow: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
        match &p.said {
            Some(s) => {
                ui.mark(c, st, if s.error { Tone::Threat } else { Tone::Good }, 0.42, 0.42);
                ui.sentence(c, st, SENTENCE, if s.error { Tone::Threat } else { Tone::Fg }, &s.text);
            }
            None if p.pending.is_some() => {
                ui.mark(c, st, Tone::Acc, 0.42, 0.42);
                ui.sentence(c, st, SENTENCE, Tone::Fg2, &self.t("asking"));
            }
            None => {
                let line = self.w.f("account.line", &[("user", &self.username()), ("hub", self.hub)]);
                ui.sentence(c, st, SENTENCE, Tone::Fg2, &line);
            }
        }
        if p.tab == Tab::Invites {
            let ready = !p.intent.trim().is_empty() && p.pending.is_none();
            let b = ui.button(c, bar, Kind::Primary, &self.t("make"), Some("Enter"), 2.4, State { disabled: !ready, ..State::NONE });
            c.entity(b).insert(AccountAction::Make);
        }
        let mut foot = vec![("↑↓", "Row"), ("Enter", "Change")];
        if tabs.len() > 1 {
            foot.push(("Q E", "Tab"));
        }
        if p.row() == Some(Row::Avatar) {
            foot.push(("←→", "Avatar"));
        }
        chimera_ui::shell::keys(ui, c, frame, &foot);
        root
    }

    /// One row: its label, the control in the one column, and below it
    /// what the row says now (a name's check, the code, the confirm).
    fn row(&mut self, c: &mut Commands, list: Entity, i: usize, r: Row) {
        let ui = self.ui;
        let p = self.p;
        let focus = p.focus == i;
        let st = State::focused(focus);
        let f = ui.form();
        let (label, note): (String, Option<String>) = match r {
            Row::Username => (self.t("username"), None),
            Row::Avatar => (self.t("avatar"), Some(format!("{} / 32", p.avatar_now(self.fallback) + 1))),
            Row::Hub => (self.t("hub"), None),
            Row::Link => (self.t("link"), None),
            Row::SignOut => (self.t("sign_out"), None),
            Row::Intent => (self.t("intent"), None),
            Row::Recipient => (self.t("recipient"), Some(self.t("optional"))),
            Row::Role => (self.t("role"), None),
            Row::Make => (self.t("invite_link"), None),
            Row::Open(id) => {
                let inv = p.invites.iter().flatten().find(|x| x.id == id);
                (
                    inv.map(|x| x.name.clone()).unwrap_or_default(),
                    inv.map(|x| format!("{} · until {}", x.intent.clone().unwrap_or_default(), day_month(x.expires))),
                )
            }
        };
        let sr = ui.setting_row(c, list, &label, RowLook { note: note.as_deref(), ..default() }, st);
        c.entity(sr.row).insert((clickable(), AccountAction::Row(i)));
        let ctl = sr.control;
        let w = f.control_w;
        match r {
            Row::Username => {
                let editing = p.editing == Some(Editing::Name);
                let value = if editing { p.name.clone() } else { format!("@{}", self.username()) };
                let folded = p.name.trim().to_ascii_lowercase();
                let taken = editing && p.name_check.as_ref().is_some_and(|n| n.username == folded && !n.available);
                let bad = editing && !folded.is_empty() && !UsernameRules::of(&folded).all();
                let fst = State { focus: editing, error: taken || bad, ..State::NONE };
                let tf = ui.text_field(c, ctl, &value, "", editing.then(|| p.caret.cursor(&p.name)), false, w, fst);
                c.entity(tf).insert((clickable(), AccountAction::Edit(Editing::Name)));
                if editing {
                    let (tone, line) = if bad {
                        (Tone::Threat, self.w.t("name.fix"))
                    } else if taken {
                        (Tone::Threat, self.w.f("name.taken", &[("hub", self.hub), ("name", &folded)]))
                    } else if p.name_ok() {
                        (Tone::AccText, self.t("name_free"))
                    } else {
                        (Tone::Fg3, self.w.f("name.checking", &[("hub", self.hub)]))
                    };
                    let under = self.under(c, list);
                    ui.mark(c, under, tone, 0.5, 0.5);
                    ui.text_plain(c, under, Role::Mono, 0.56, tone, &line.to_uppercase());
                    if taken {
                        ui.text(c, under, Role::Mono, 0.56, Tone::Fg3, &self.w.t("name.free"));
                        for s in p.name_check.as_ref().map(|n| n.suggestions.clone()).unwrap_or_default() {
                            let b = ui.button(c, under, Kind::Secondary, &s, None, 1.7, State::NONE);
                            c.entity(b).insert(AccountAction::Suggest(s.clone()));
                        }
                    }
                }
            }
            Row::Avatar => {
                let a = p.avatar_now(self.fallback);
                let line = ui.boxed(c, ctl, UiStyle { w: Some(w), gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
                let av = self.avatar(c, line, a, 2.0);
                c.entity(av).insert(Pickable::IGNORE);
                ui.text_plain(c, line, Role::DisplayBold, 0.86, Tone::Fg, &self.display());
            }
            Row::Hub => {
                let line = ui.boxed(c, ctl, UiStyle { w: Some(w), gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
                ui.mark(c, line, if self.offline { Tone::Warn } else { Tone::Good }, 0.6, 0.6);
                ui.text_plain(c, line, Role::DisplayBold, 0.86, Tone::Fg, self.hub);
            }
            Row::Link => {
                let b = ui.button(c, ctl, Kind::Secondary, &self.t(if p.link.is_some() { "link_again" } else { "link_show" }), None, f.control_h, State { focus, ..State::NONE });
                c.entity(b).insert(AccountAction::Link);
                if let Some((code, minutes)) = &p.link {
                    let under = self.under(c, list);
                    let cells = ui.boxed(c, under, UiStyle { gap: Some(0.35), align: Some(AlignItems::Center), ..UiStyle::row() });
                    for (k, ch) in code.chars().enumerate() {
                        if ch == '-' {
                            ui.text_plain(c, cells, Role::Mono, 1.2, Tone::Fg3, "–");
                            continue;
                        }
                        let cell = ui.boxed(c, cells, UiStyle { w: Some(2.1), h: Some(2.6), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), ..UiStyle::row() }.bg_a(Tone::Fg, 0.05).outline(if k == 0 { Tone::Acc } else { Tone::Fg4 }, 1.0));
                        ui.text_plain(c, cell, Role::Mono, 1.3, Tone::Fg, &ch.to_string());
                    }
                    let words = ui.boxed(c, under, UiStyle { gap: Some(0.3), margin: Some([0.0, 0.0, 0.0, 1.0].into()), ..UiStyle::column() });
                    ui.text_plain(c, words, Role::Mono, 0.56, Tone::AccText, &self.w.f("account.link_valid", &[("n", &minutes.to_string())]).to_uppercase());
                    ui.sentence(c, words, 0.8, Tone::Fg3, &self.t("link_where"));
                }
            }
            Row::SignOut => {
                if p.confirm_sign_out {
                    let two = ui.boxed(c, ctl, UiStyle { w: Some(w), gap: Some(0.5), ..UiStyle::row() });
                    let stay = ui.button(c, two, Kind::Secondary, &self.t("stay"), Some("Esc"), f.control_h, State::NONE);
                    c.entity(stay).insert(AccountAction::SignOutStay);
                    let go = ui.button(c, two, Kind::Danger, &self.t("sign_out_confirm"), None, f.control_h, State { focus: true, ..State::NONE });
                    c.entity(go).insert(AccountAction::SignOutConfirm);
                    let under = self.under(c, list);
                    ui.mark(c, under, Tone::Warn, 0.5, 0.5);
                    let line = self.w.f("account.sign_out_ask", &[("user", &self.username()), ("hub", self.hub)]);
                    let words = ui.boxed(c, under, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
                    ui.paragraph(c, words, Role::Text, 0.84, Tone::Fg, &line);
                } else {
                    let b = ui.button(c, ctl, Kind::Danger, &self.t("sign_out_button"), None, f.control_h, State { focus, ..State::NONE });
                    c.entity(b).insert(AccountAction::SignOut);
                }
            }
            Row::Intent | Row::Recipient => {
                let (e, text, caret, ph) = if r == Row::Intent {
                    (Editing::Intent, &p.intent, p.icaret, self.t("intent_placeholder"))
                } else {
                    (Editing::Recipient, &p.recipient, p.rcaret, self.t("recipient_placeholder"))
                };
                let editing = p.editing == Some(e);
                let tf = ui.text_field(c, ctl, text, &ph, editing.then(|| caret.cursor(text)), false, w, State::focused(editing));
                c.entity(tf).insert((clickable(), AccountAction::Edit(e)));
            }
            Row::Role => {
                let options = [self.t("role_tester"), self.t("role_host")];
                let refs: Vec<&str> = options.iter().map(String::as_str).collect();
                if let Choice::Segmented(segs) = ui.choice(c, ctl, &refs, Some(p.role), None, st) {
                    for (k, e) in segs.into_iter().enumerate() {
                        c.entity(e).insert(AccountAction::Role(k));
                    }
                }
            }
            Row::Make => {
                match &p.made {
                    Some(m) => {
                        let b = ui.button(c, ctl, Kind::Secondary, &self.t("copy"), Some("Ctrl C"), f.control_h, State { focus, ..State::NONE });
                        c.entity(b).insert(AccountAction::CopyLink);
                        // The link, once, on a plate across the list.
                        let plate = ui.boxed(c, list, UiStyle { gap: Some(0.5), pad: Some([1.0, 1.4].into()), margin: Some([0.2, 0.0, 0.6, 0.0].into()), shrink: Some(0.0), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.8).outline(Tone::AccText, 0.4));
                        ui.corners(c, plate, Tone::AccText, 0.9);
                        let head = ui.boxed(c, plate, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
                        ui.badge(c, head, &m.name, BadgeKind::Hero);
                        ui.text_plain(c, head, Role::Mono, 0.56, Tone::AccText, &self.w.f("account.made_note", &[("date", &day_month(m.expires))]).to_uppercase());
                        let link = m.link.clone().unwrap_or_else(|| m.code.clone());
                        ui.text_plain(c, plate, Role::Mono, 0.8, Tone::Fg, &short_link(&link));
                    }
                    None => {
                        ui.text_plain(c, ctl, Role::Mono, 0.56, Tone::Fg3, &self.t("make_hint").to_uppercase());
                    }
                }
            }
            Row::Open(id) => {
                let inv = p.invites.iter().flatten().find(|x| x.id == id);
                let line = ui.boxed(c, ctl, UiStyle { w: Some(w), gap: Some(0.6), align: Some(AlignItems::Center), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
                if let Some(x) = inv {
                    ui.badge(c, line, &x.role, if x.role == "host" { BadgeKind::Hero } else { BadgeKind::Quiet });
                }
                let b = ui.button(c, line, Kind::Secondary, &self.t("revoke"), None, f.control_h, State { focus, ..State::NONE });
                c.entity(b).insert(AccountAction::Revoke(id));
            }
        }
    }

    /// A line under a row (its check, its code, its confirm), indented.
    fn under(&self, c: &mut Commands, list: Entity) -> Entity {
        self.ui.boxed(c, list, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), pad: Some([0.4, 1.4, 0.8, 1.4].into()), shrink: Some(0.0), w_pct: Some(100.0), ..UiStyle::row() })
    }

    /// The help tile at the right: the focused row's title, what it does
    /// and its facts; the avatar's 32 looks on the avatar row.
    fn help(&mut self, c: &mut Commands, cols: Entity, side_w: f32) {
        let ui = self.ui;
        let p = self.p;
        let help = ui.boxed(c, cols, UiStyle { w: Some(side_w), gap: Some(1.0), shrink: Some(0.0), ..UiStyle::column() });
        let Some(r) = p.row() else { return };
        let key = match r {
            Row::Username => "username",
            Row::Avatar => "avatar",
            Row::Hub => "hub",
            Row::Link => "link",
            Row::SignOut => "sign_out",
            Row::Intent => "intent",
            Row::Recipient => "recipient",
            Row::Role => "role",
            Row::Make => "invite_link",
            Row::Open(_) => "open",
        };
        let tagrow = ui.boxed(c, help, UiStyle::row());
        let (node, label) = chimera_ui::widgets::tag(ui.theme, ui.kit, &ui.units, chimera_ui::widgets::TagKind::Accent, "Selected", 0.5);
        let tag = c.spawn((node, ChildOf(tagrow))).id();
        c.spawn((label, ChildOf(tag)));
        let title = match r {
            Row::Open(id) => p.invites.iter().flatten().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default(),
            _ => self.t(key),
        };
        ui.text(c, help, Role::DisplayBold, 1.3, Tone::Fg, &title);
        let tx = ui.boxed(c, help, UiStyle { w_pct: Some(100.0), ..UiStyle::column() });
        let text = self.w.f(&format!("account.help.{key}"), &[("hub", self.hub), ("user", &self.username())]);
        ui.paragraph(c, tx, Role::Text, SENTENCE, Tone::Fg2, &text);
        let me = p.me.clone().unwrap_or_default();
        let kv: Vec<(&str, String)> = match r {
            Row::Username => {
                let mut v = vec![("Username", format!("@{}", self.username())), ("Shown as", self.display())];
                if !me.previous_usernames.is_empty() {
                    v.push(("Was", me.previous_usernames.iter().take(3).map(|u| format!("@{u}")).collect::<Vec<_>>().join(" ")));
                }
                v.push(("Renames", "One a minute".into()));
                v
            }
            Row::Hub => vec![
                ("Hub", self.hub.to_string()),
                ("Address", self.host.to_string()),
                ("Status", if self.offline { "Offline".into() } else { "Online".into() }),
                ("Role", if me.role.is_empty() { "-".into() } else { me.role.clone() }),
            ],
            Row::Open(id) => p
                .invites
                .iter()
                .flatten()
                .find(|x| x.id == id)
                .map(|x| {
                    vec![
                        ("Intent", x.intent.clone().unwrap_or_default()),
                        ("For", x.recipient.clone().unwrap_or_else(|| "-".into())),
                        ("Role", x.role.clone()),
                        ("Made", day_month(x.created)),
                        ("Until", day_month(x.expires)),
                    ]
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if !kv.is_empty() {
            let kv: Vec<(&str, String)> = kv.into_iter().map(|(k, v)| (k, if k == "Username" || k == "Was" || k == "Address" { v } else { v.to_uppercase() })).collect();
            ui.kv(c, help, &kv, 7.0);
        }
        if r == Row::Avatar {
            // The 32 looks: one marker, the chosen one outlined.
            let chosen = p.avatar_now(self.fallback);
            let grid = ui.boxed(c, help, UiStyle { gap: Some(0.35), margin: Some([0.4, 0.0, 0.0, 0.0].into()), ..UiStyle::column() });
            for row in 0..4u8 {
                let line = ui.boxed(c, grid, UiStyle { gap: Some(0.35), ..UiStyle::row() });
                for col in 0..8u8 {
                    let b = row * 8 + col;
                    let sel = b == chosen;
                    let cell = ui.boxed(c, line, UiStyle { pad: Some([0.15, 0.15].into()), ..UiStyle::row() }.outline(if sel { Tone::Acc } else { Tone::Fg5 }, if sel { 2.0 } else { 1.0 }));
                    c.entity(cell).insert((clickable(), AccountAction::Avatar(b)));
                    let a = self.avatar(c, cell, b, 2.4);
                    c.entity(a).insert(Pickable::IGNORE);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Staged scenes.

fn scene(w: &mut World, args: &[String]) -> Result<(), String> {
    let what = args.first().map(String::as_str).unwrap_or("profile");
    let owner = what != "player";
    w.insert_resource(AccountBackend::staged(owner));
    w.insert_resource(OnboardingFiles::default());
    if what == "off" {
        w.resource_mut::<AccountPage>().open = false;
        return Ok(());
    }
    if w.get_resource::<AccountShown>().is_none_or(|s| s.0.is_none()) {
        w.insert_resource(AccountShown(Some(Shown { display: "Sam Rocket".into(), username: "sam_rocket".into(), hub: "Friday Night Hub".into() })));
    }
    w.resource_mut::<AccountPage>().me = None;
    w.resource_mut::<AccountPage>().invites = None;
    open_page(w);
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let staged = StagedAccount::new(owner);
    let mut p = w.resource_mut::<AccountPage>();
    p.since = now - 10.0;
    match what {
        "profile" | "player" => {}
        "rename" | "rename-taken" => {
            let name = if what == "rename" { "sam_rocket_gg" } else { "rocket" };
            p.editing = Some(Editing::Name);
            p.name = name.into();
            p.caret = Caret::end_of(name);
            p.asked_name = Some(name.into());
            p.name_check = staged.check_rename(name).ok();
        }
        "avatar" => p.focus = 1,
        "link" => {
            p.focus = 3;
            p.link = Some((STAGED_LINK.into(), 10));
        }
        "sign-out" => {
            p.focus = 4;
            p.confirm_sign_out = true;
        }
        "invites" | "invite-made" => {
            p.tab = Tab::Invites;
            p.intent = "sent on Discord".into();
            p.recipient = "Alex".into();
            if what == "invite-made" {
                let request = InviteRequest { intent: "sent on Discord".into(), recipient: Some("Alex".into()), role: Some("tester".into()) };
                p.made = staged.make_invite(&request).ok();
                p.invites = staged.open_invites().ok();
                p.intent.clear();
                p.recipient.clear();
                p.focus = 3;
                p.said = Some(Said { error: false, text: "Invite for Alex made: send the link privately.".into() });
            } else {
                p.editing = Some(Editing::Intent);
                p.icaret = Caret::end_of("sent on Discord");
            }
        }
        other => {
            return Err(format!(
                "ui_scene account {other}: profile, rename, rename-taken, avatar, link, sign-out, invites, invite-made, player or off"
            ));
        }
    }
    Ok(())
}
