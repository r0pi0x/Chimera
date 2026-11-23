//! Settings > Account (feature `present`; docs/plans/active/onboarding.md,
//! "The account page"): this PC's hub account, opened by a click on
//! your own avatar in the party strip (`ShellTile::Profile`, on any shell
//! screen; `open_from_avatar`) or P on the main menu (`open_from_key`);
//! no card in the corner (the user's report, 2026-11-22). A shell
//! screen in Settings' look (the bar's crumb "Main menu ‹ Account", the
//! window, the grey-to-white tabs with badges, one row marker, each row's
//! control in the one control column, the focused row's help in the tile
//! at the right, the action bar), its words in `data/onboarding.toml
//! [account]`.
//!
//! - **Ground**: the main menu's (`chimera_ui::shell::menu_ground`).
//! - **Profile** (the Form Kit mockup, `chimera_ui::form_page`): at the
//!   left what can be changed, the username (a form field: title, help,
//!   status slot; its availability asked of the hub `NAME_PAUSE` after the
//!   last key, a taken name's free names offered as fixes), the display
//!   name (optional: empty is the username; `codes::display_name`'s rules
//!   checked as typed, the hub's answer too: never another's username)
//!   and the 32 avatars (`avatar_grid`); the changes collect in the action bar ("1
//!   change · avatar", from two on "N changes" with the list unfolding)
//!   and go out together with **Save changes** (Ctrl+S, Enter in the
//!   name; a name known taken stops on the field), Discard puts them
//!   back; while it saves every action waits. At the right only what
//!   can't be changed here: the hub, its address masked (eye and Copy,
//!   `REVEAL_HOST`), the role, then **Link another device** (a 10-minute
//!   code for "I already have an account" on the other PC,
//!   `account::link_code`) and **Sign out** (asks in place: Stay or Sign
//!   out; then this device is revoked at the hub when it answers,
//!   `account.toml` deleted either way, and the first run's Welcome
//!   shows).
//! - **Invites** (an owner's account only, by `/accounts/me`'s role): the
//!   form (why, for, role: Tester or Host) with Make invite (busy
//!   "Making invite…"), which becomes the result card (the link shown
//!   once, Copy link, Make another) once made; beside it the open invites
//!   (newest first, each with Revoke, which asks in its row: Keep or
//!   Revoke invite), `account::make_invite`, `open_invites`,
//!   `revoke_invite`. The tab's badge counts the open ones.
//! - **Keys**: ↑↓ rows, Enter acts (edits a field, commits it), Esc
//!   leaves a field or the confirm, else closes (the crumb and a right
//!   click too); Q and E the tabs; ←→ the avatar and the role. Nothing
//!   animates in a way that holds a key back.
//! - **Calls** go through `AccountBackend` (`NetworkAccount`: the live
//!   session's ticket, on a thread; `StagedAccount` answers in the same
//!   frame for tests and `ui_scene account <profile|rename|rename-taken|display|
//!   avatar|saving|link|sign-out|invites|invite-made|player>`).
//!
//! A saved display name asks a new ticket at once (`Session::refresh`), so
//! the party's hub shows it to the others at the next heartbeat.
//!
//! Known shortcuts: no device list or new recovery key here yet
//! (hub.md 8); the link code's "valid for" is drawn once, not counted
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
    controls::{self, Kind, Size, State, Ui},
    form::Choice,
    form_page::{BadgeLook, FieldSpec, FieldStatus, FooterAction, FooterSpec, ResultSpec, RowValue},
    motion::UiClock,
    text_edit::{Caret, text_key},
};

use crate::{
    main_menu::MenuUiCamera,
    onboarding::{AccountShown, Onboarding, OnboardingFiles, Shown, Words},
};

/// Over the main menu (Settings' place: never both).
pub const ACCOUNT_Z: i32 = 92;
/// The hub's address's reveal key (`chimera_ui::secret`).
pub const REVEAL_HOST: &str = "account.host";
/// Seconds after the last key before a new username is asked of the hub.
pub const NAME_PAUSE: f64 = crate::onboarding::NAME_PAUSE;
/// The roles an invite made here may carry (an owner's: the command line).
pub const ROLES: [&str; 2] = ["tester", "host"];
/// The made invite's Copy key (`form_page::Copies`).
pub const COPY_INVITE: &str = "account.invite";
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
    Display,
    Avatar,
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
    Display,
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
    /// The display name as typed (the account's until changed).
    pub display: String,
    pub dcaret: Caret,
    /// The avatar picked here, not saved yet (Save changes sends it).
    pub avatar: Option<u8>,
    /// A save under way (Save changes): every action waits.
    pub saving: bool,
    /// The footer's list of changes unfolded.
    pub changes_open: bool,
    /// The open invite whose Revoke asks (its row asks in place).
    pub confirm_revoke: Option<i64>,
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
            Tab::Profile => vec![Row::Username, Row::Display, Row::Avatar, Row::Link, Row::SignOut],
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

    /// The typed username differs from the account's.
    pub fn name_changed(&self) -> bool {
        let n = self.name.trim();
        !n.is_empty() && self.me.as_ref().is_some_and(|m| !n.eq_ignore_ascii_case(&m.username))
    }

    /// The display name to send: as typed with white space folded, the
    /// username when left empty.
    pub fn display_wanted(&self) -> String {
        let d = self.display.split_whitespace().collect::<Vec<_>>().join(" ");
        if d.is_empty() { self.me.as_ref().map(|m| m.username.clone()).unwrap_or_default() } else { d }
    }

    /// The typed display name differs from the account's.
    pub fn display_changed(&self) -> bool {
        self.me.as_ref().is_some_and(|m| self.display_wanted() != m.display_name)
    }

    /// Why the typed display name can't be had (`codes::display_name`).
    pub fn display_problem(&self) -> Option<String> {
        if !self.display_changed() {
            return None;
        }
        crate::onboarding::display_problem(&self.display_wanted())
    }

    /// The picked avatar differs from the account's.
    pub fn avatar_changed(&self) -> bool {
        self.avatar.is_some() && self.avatar != self.me.as_ref().and_then(|m| m.avatar)
    }

    /// The Profile's changes not saved yet, one short name each.
    pub fn changes(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.name_changed() {
            v.push("username".to_string());
        }
        if self.display_changed() {
            v.push("display name".to_string());
        }
        if self.avatar_changed() {
            v.push("avatar".to_string());
        }
        v
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
            self.confirm_revoke = None;
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
    fn set_display_name(&self, name: &str) -> Result<Me, String>;
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
    fn set_display_name(&self, name: &str) -> Result<Me, String> {
        let (f, t) = session()?;
        let me = account::view(account::set_display_name(&f, &t, name)?)?;
        // The party's hub reads names from tickets: a new one says it.
        if let Some(s) = account::live() {
            let _ = s.refresh();
        }
        Ok(me)
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
    fn set_display_name(&self, name: &str) -> Result<Me, String> {
        let d = codes::display_name(name).map_err(|e| format!("the hub refused: {e} (400)"))?;
        let folded = d.to_lowercase().replace(' ', "_");
        if self.taken.iter().any(|t| *t == folded) {
            return Err(format!("the hub refused: {d:?} is another player's username: pick another display name (400)"));
        }
        let mut s = self.lock();
        s.0.display_name = d;
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
    /// Save changes: the new name, the display name, then the avatar.
    Save { name: Option<String>, display: Option<String>, avatar: Option<u8> },
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
    /// Saved, with the avatar sent (if any).
    Saved(Result<Me, String>, Option<u8>),
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
    if !matches!(what, Ask::Me | Ask::Check(_) | Ask::Invites) {
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
            Ask::Save { name, display, avatar } => {
                let mut r = match &name {
                    Some(n) => hub.rename(n),
                    None => hub.me(),
                };
                if let (Ok(_), Some(d)) = (&r, &display) {
                    r = hub.set_display_name(d);
                }
                if let (Ok(_), Some(a)) = (&r, avatar) {
                    r = hub.set_avatar(a);
                }
                Reply::Saved(r, avatar)
            }
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
    /// The result card's Make another: back to the form.
    Another,
    /// Revoke asks in its row; Confirm revokes, Keep backs off.
    Revoke(i64),
    RevokeConfirm(i64),
    RevokeKeep,
    /// The Profile's Save changes (Ctrl+S, Enter in the name) and Discard.
    Save,
    Discard,
    /// The footer's list of changes, unfolded or folded.
    ToggleChanges,
    Close,
}

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
        app.add_systems(Update, (open_from_avatar, open_from_key, clicks, keys, poll, pauses, act, show).chain());
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

/// Your own avatar in the party strip (`ShellTile::Profile`), on any
/// shell screen, opens the page (the user's report, 2026-11-22). Not
/// from the page itself, and only with an account.
fn open_from_avatar(
    pressed: Query<(&Interaction, &chimera_ui::shell::ShellTile), Changed<Interaction>>,
    page: Res<AccountPage>,
    shown: Res<crate::onboarding::AccountShown>,
    mut commands: Commands,
) {
    if page.open || shown.0.is_none() {
        return;
    }
    if pressed.iter().any(|(i, t)| *i == Interaction::Pressed && *t == chimera_ui::shell::ShellTile::Profile) {
        commands.queue(open_page);
    }
}

/// P on the main menu opens the page too (the keyboard's way to what the
/// avatar opens; a controller reaches it by the avatar's tile).
#[allow(clippy::too_many_arguments)]
fn open_from_key(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    menu: Option<Res<crate::main_menu::PartyMenu>>,
    shown: Res<crate::onboarding::AccountShown>,
    console: Option<Res<chimera_client::client::console::ConsoleUi>>,
    (page, party, onboarding): (Res<AccountPage>, Option<Res<crate::party_signal::PartyPanel>>, Option<Res<crate::onboarding::Onboarding>>),
    mut commands: Commands,
) {
    let on_menu = menu.as_deref().is_some_and(|m| m.shown() && m.screen == crate::main_menu::Screen::MainMenu && m.page == crate::main_menu::MenuPage::Main && m.failure.is_none());
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::KeyP))
        || !on_menu
        || page.open
        || shown.0.is_none()
        || party.is_some_and(|p| p.open)
        || crate::onboarding::takes_keys(onboarding.as_deref())
        || console.is_some_and(|c| c.open)
    {
        return;
    }
    commands.queue(open_page);
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
                if p.confirm_revoke.take().is_some() || p.editing.take().is_some() {
                } else if p.confirm_sign_out {
                    p.confirm_sign_out = false;
                } else {
                    out.write(AccountAction::Close);
                }
                continue;
            }
            Key::Enter => {
                match p.editing {
                    // Enter in a name saves the Profile's changes.
                    Some(Editing::Name | Editing::Display) => {
                        out.write(AccountAction::Save);
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
                    Some(Row::Role) if p.made.is_none() => {
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
            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("s") && p.tab == Tab::Profile => {
                out.write(AccountAction::Save);
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
            Editing::Display => {
                let mut caret = p.dcaret;
                if caret.key(&mut p.display, tk, clipboard, codes::NAME_MAX, false) {
                    p.said = None;
                }
                p.dcaret = caret;
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

/// The name asked of the hub after a pause.
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
                if p.editing.is_none() && !p.name_changed() {
                    p.name = me.username.clone();
                }
                if p.editing != Some(Editing::Display) && (p.me.is_none() || !p.display_changed()) {
                    p.display = me.display_name.clone();
                }
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
            Reply::Saved(Ok(me), avatar) => {
                keep_names(&mut commands, &me, backend.inline);
                if let Some(a) = avatar {
                    // The look in games follows the saved one.
                    commands.queue(move |w: &mut World| {
                        if let Some(mut c) = w.get_resource_mut::<chimera_core::console::Console>() {
                            c.submit(format!("cl_avatar {a}"));
                        }
                    });
                }
                p.said = Some(Said { error: false, text: words.f("account.saved", &[("user", &me.username)]) });
                p.name = me.username.clone();
                p.display = me.display_name.clone();
                p.me = Some(me);
                p.avatar = None;
                p.saving = false;
                p.changes_open = false;
                p.editing = None;
                p.name_check = None;
            }
            Reply::Saved(Err(e), _) => {
                p.saving = false;
                p.said = Some(err(&e));
            }
            Reply::Link(Err(e)) | Reply::Made(Err(e)) | Reply::Invites(Err(e)) => p.said = Some(err(&e)),
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
                    p.confirm_revoke = None;
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
            && (s.file().username != me.username || (!me.display_name.is_empty() && s.file().display_name != me.display_name))
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
    mut copies: Option<ResMut<chimera_ui::form_page::Copies>>,
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
                        Some(Editing::Name) => save(&mut p, &backend, &answers, &words),
                        _ => start_name(&mut p, now),
                    },
                    Some(Row::Display) => match p.editing {
                        Some(Editing::Display) => save(&mut p, &backend, &answers, &words),
                        _ => start_display(&mut p),
                    },
                    Some(Row::Intent) => edit(&mut p, Editing::Intent),
                    Some(Row::Recipient) => edit(&mut p, Editing::Recipient),
                    Some(Row::Link) if p.pending.is_none() => ask(&mut p, &backend, &answers, Ask::Link),
                    Some(Row::SignOut) if p.confirm_sign_out && p.pending.is_none() => ask(&mut p, &backend, &answers, Ask::SignOut),
                    Some(Row::SignOut) => p.confirm_sign_out = true,
                    Some(Row::Role) => p.role = (p.role + 1) % ROLES.len(),
                    Some(Row::Make) if p.made.is_some() => copy_link(&mut p, &words, copies.as_deref_mut(), clipboard.as_deref_mut(), now),
                    Some(Row::Make) => make(&mut p, &backend, &answers, &words),
                    Some(Row::Open(id)) if p.pending.is_none() => {
                        let id = *id;
                        if p.confirm_revoke == Some(id) {
                            ask(&mut p, &backend, &answers, Ask::Revoke(id));
                        } else {
                            p.confirm_revoke = Some(id);
                        }
                    }
                    _ => {}
                }
            }
            AccountAction::Edit(Editing::Name) => {
                p.focus = 0;
                start_name(&mut p, now);
            }
            AccountAction::Edit(Editing::Display) => {
                p.focus = rows.iter().position(|r| *r == Row::Display).unwrap_or(p.focus);
                start_display(&mut p);
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
                if p.saving {
                    continue;
                }
                // Picked here; Save changes sends it.
                p.avatar = (p.me.as_ref().and_then(|m| m.avatar) != Some(b)).then_some(b);
                p.focus = rows.iter().position(|r| *r == Row::Avatar).unwrap_or(p.focus);
                p.said = None;
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
            AccountAction::CopyLink => copy_link(&mut p, &words, copies.as_deref_mut(), clipboard.as_deref_mut(), now),
            AccountAction::Another => {
                p.made = None;
                p.focus = 0;
                p.said = None;
            }
            AccountAction::Revoke(id) => {
                p.confirm_revoke = Some(id);
                p.focus = rows.iter().position(|r| *r == Row::Open(id)).unwrap_or(p.focus);
            }
            AccountAction::RevokeConfirm(id) => {
                if p.pending.is_none() && p.confirm_revoke == Some(id) {
                    ask(&mut p, &backend, &answers, Ask::Revoke(id));
                }
            }
            AccountAction::RevokeKeep => p.confirm_revoke = None,
            AccountAction::Save => save(&mut p, &backend, &answers, &words),
            AccountAction::Discard => {
                if !p.saving {
                    p.name = p.me.as_ref().map(|m| m.username.clone()).unwrap_or_default();
                    p.display = p.me.as_ref().map(|m| m.display_name.clone()).unwrap_or_default();
                    p.avatar = None;
                    p.editing = None;
                    p.name_check = None;
                    p.changes_open = false;
                    p.said = None;
                }
            }
            AccountAction::ToggleChanges => p.changes_open = !p.changes_open,
        }
    }
}

fn start_name(p: &mut AccountPage, now: f64) {
    if p.saving {
        return;
    }
    let current = p.me.as_ref().map(|m| m.username.clone()).unwrap_or_default();
    if !p.name_changed() {
        p.name = current.clone();
        p.name_check = Some(NameCheck { username: current.clone(), available: true, problem: None, suggestions: vec![] });
        p.asked_name = Some(current);
        p.typed_at = now;
    }
    p.caret = Caret::end_of(&p.name);
    p.editing = Some(Editing::Name);
    p.confirm_sign_out = false;
}

/// The display name's field takes the keys.
fn start_display(p: &mut AccountPage) {
    if p.saving {
        return;
    }
    p.dcaret = Caret::end_of(&p.display);
    p.editing = Some(Editing::Display);
    p.confirm_sign_out = false;
}

/// Save changes: the new name (refused here when it is known taken or
/// breaks the rules: "pick a free username"), the display name (refused
/// here when it breaks `codes::display_name`), then the avatar, in one
/// call; every action waits meanwhile (`saving`).
fn save(p: &mut AccountPage, backend: &AccountBackend, answers: &Answers, words: &Words) {
    if p.pending.is_some() || p.saving {
        return;
    }
    if p.changes().is_empty() {
        p.editing = None;
        return;
    }
    let name = p.name_changed().then(|| p.name.trim().to_ascii_lowercase());
    if let Some(n) = &name {
        let taken = p.name_check.as_ref().is_some_and(|c| c.username == *n && !c.available);
        if taken || !UsernameRules::of(n).all() {
            p.said = Some(Said { error: true, text: words.t("account.save_blocked") });
            p.focus = 0;
            p.editing = Some(Editing::Name);
            return;
        }
    }
    if p.display_problem().is_some() {
        p.said = Some(Said { error: true, text: words.t("account.display_blocked") });
        p.focus = p.rows().iter().position(|r| *r == Row::Display).unwrap_or(p.focus);
        p.editing = Some(Editing::Display);
        p.dcaret = Caret::end_of(&p.display);
        return;
    }
    let display = p.display_changed().then(|| p.display_wanted());
    let avatar = p.avatar.filter(|_| p.avatar_changed());
    p.editing = None;
    p.saving = true;
    p.said = None;
    ask(p, backend, answers, Ask::Save { name, display, avatar });
}

/// Copy link: the made invite's link on the clipboard, the card's Copy
/// reading "Copied" (`form_page::Copies`).
fn copy_link(p: &mut AccountPage, words: &Words, copies: Option<&mut chimera_ui::form_page::Copies>, clipboard: Option<&mut chimera_ui::clipboard::Clipboard>, now: f64) {
    if let (Some(m), Some(c)) = (p.made.as_ref(), clipboard) {
        c.set(m.link.as_deref().unwrap_or(&m.code));
        p.said = Some(Said { error: false, text: words.t("account.copied") });
        if let Some(k) = copies {
            k.mark(COPY_INVITE, now);
        }
    }
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
        Editing::Name | Editing::Display => {}
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

type DrawKey = (AccountPage, Option<Shown>, (String, bool), i32, (u32, u32), Option<Entity>, chimera_ui::shell::Party, (chimera_ui::secret::Reveals, chimera_ui::form_page::Copies));

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
    (theme, kit, units, reveals, copies): (
        Option<Res<chimera_ui::Theme>>,
        Option<Res<chimera_ui::KitFonts>>,
        Option<Res<chimera_ui::UiUnits>>,
        Option<Res<chimera_ui::secret::Reveals>>,
        Option<Res<chimera_ui::form_page::Copies>>,
    ),
    (mut images, mut icons, mut ambients): (Option<ResMut<Assets<Image>>>, Option<ResMut<chimera_ui::icons::IconImages>>, Option<ResMut<Assets<chimera_ui::AmbientMaterial>>>),
    mut drawn: Local<Option<DrawKey>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let cam = camera.iter().next();
    let key = p.open.then(|| {
        let mut snap = p.clone();
        snap.typed_at = 0.0;
        snap.asked_name = None;
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
            (reveals.as_deref().cloned().unwrap_or_default(), copies.as_deref().cloned().unwrap_or_default()),
        )
    });
    if *drawn == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((_, shown, (hub_name, offline), look, _, cam, party, (reveals, copies))) = key.clone() {
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
            reveals: &reveals,
            copies: &copies,
            images: images.as_deref_mut(),
            icons: icons.as_deref_mut(),
        };
        let r = d.screen(&mut commands, &party, ambients.as_deref_mut());
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
    reveals: &'a chimera_ui::secret::Reveals,
    copies: &'a chimera_ui::form_page::Copies,
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

    fn role(&self) -> String {
        let r = self.p.me.as_ref().map(|m| m.role.clone()).unwrap_or_default();
        let mut c = r.chars();
        c.next().map_or_else(|| "-".to_string(), |f| f.to_uppercase().chain(c).collect())
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

    fn screen(&mut self, c: &mut Commands, party: &chimera_ui::shell::Party, ambients: Option<&mut Assets<chimera_ui::AmbientMaterial>>) -> Entity {
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
        // The ground: the main menu's and Settings'.
        chimera_ui::shell::menu_ground(c, root, t, ambients);
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
        let cols = ui.boxed(c, win, UiStyle { gap: Some(2.0 * sh.rows_in), align: Some(AlignItems::Stretch), ..UiStyle::row() }.abs([Some(sh.rows_in), Some(4.6), Some(sh.rows_in), Some(sh.act_h + 1.0)]));
        let sc = ui.scroll(c, cols, SCROLL_KEY, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() }, UiStyle { gap: Some(1.2), ..UiStyle::column() });
        let list = sc.content;
        // The list's width: the window's less the tile (Profile) and the insets.
        let win_w = 100.0 - 2.0 * sh.margin - 2.0 * sh.rows_in - 1.0;
        let bar = chimera_ui::shell::action_bar(ui, c, win);
        match p.tab {
            Tab::Profile => {
                let list_w = win_w - sh.side_w - 2.0 * sh.rows_in;
                self.profile(c, list, list_w);
                self.tile(c, cols, sh.side_w);
                self.profile_footer(c, bar);
            }
            Tab::Invites => {
                self.invites(c, list, win_w);
                self.invites_footer(c, bar);
            }
        }
        let mut foot = vec![("↑↓", "Row"), ("Enter", "Change")];
        if tabs.len() > 1 {
            foot.push(("Q E", "Tab"));
        }
        if p.row() == Some(Row::Avatar) {
            foot.push(("←→", "Avatar"));
        }
        if p.tab == Tab::Profile {
            foot.push(("Ctrl+S", "Save"));
        }
        foot.push(("Esc", "Main menu"));
        chimera_ui::shell::keys(ui, c, frame, &foot);
        root
    }

    fn focused(&self, r: Row) -> bool {
        self.p.row() == Some(r)
    }

    /// Profile at the left: the name, then the avatar beside it (what you
    /// can change).
    fn profile(&mut self, c: &mut Commands, list: Entity, list_w: f32) {
        let ui = self.ui;
        let p = self.p;
        let two = ui.boxed(c, list, UiStyle { gap: Some(2.4), align: Some(AlignItems::FlexStart), pad: Some([0.6, 0.0, 0.0, 0.4].into()), ..UiStyle::row() });
        let fw = ui.theme.layout_opt("account", "field_w").unwrap_or(24.0);
        let left = ui.boxed(c, two, UiStyle { w: Some(fw), gap: Some(0.8), shrink: Some(0.0), ..UiStyle::column() });
        ui.form_section(c, left, &self.t("section_name"), None, None);
        let editing = p.editing == Some(Editing::Name);
        let folded = p.name.trim().to_ascii_lowercase();
        let changed = p.name_changed();
        let value = if editing || changed { p.name.clone() } else { self.username() };
        let taken = changed && p.name_check.as_ref().is_some_and(|n| n.username == folded && !n.available);
        let bad = (editing || changed) && !folded.is_empty() && !UsernameRules::of(&folded).all();
        let fixes = if taken { p.name_check.as_ref().map(|n| n.suggestions.clone()).unwrap_or_default() } else { Vec::new() };
        let fix_text = self.w.t("name.fix");
        let taken_text = format!("{} · {}", self.w.f("name.taken", &[("hub", self.hub), ("name", &folded)]), self.w.t("name.free"));
        let free_text = self.w.f("account.name_free", &[("was", &self.username())]);
        let checking_text = self.w.f("name.checking", &[("hub", self.hub)]);
        let hint = self.t("username_hint");
        let status = if p.saving && changed {
            FieldStatus::Checking(&checking_text)
        } else if bad {
            FieldStatus::Error { text: &fix_text, fixes: &[] }
        } else if taken {
            FieldStatus::Error { text: &taken_text, fixes: &fixes }
        } else if changed && p.name_ok() {
            FieldStatus::Ok(&free_text)
        } else if changed {
            FieldStatus::Checking(&checking_text)
        } else {
            FieldStatus::Hint(&hint)
        };
        let help = self.w.f("account.username_help", &[("hub", self.hub)]);
        let label = self.t("username");
        let tag = self.t("required");
        let ff = ui.form_field(
            c,
            left,
            &FieldSpec {
                label: &label,
                tag: Some(&tag),
                help: &help,
                value: &value,
                caret: editing.then(|| p.caret.cursor(&p.name)),
                focus: self.focused(Row::Username),
                status,
                w: fw,
                ..default()
            },
        );
        c.entity(ff.input).insert(AccountAction::Edit(Editing::Name));
        for (e, s) in ff.fixes.iter().zip(&fixes) {
            c.entity(*e).insert(AccountAction::Suggest(s.clone()));
        }
        // The display name: what games show; empty is the username.
        let editing = p.editing == Some(Editing::Display);
        let problem = p.display_problem();
        let dhint = self.t("display_hint");
        let dstatus = match &problem {
            Some(e) => FieldStatus::Error { text: e, fixes: &[] },
            None => FieldStatus::Hint(&dhint),
        };
        let (dlabel, dtag) = (self.t("display"), self.t("optional"));
        let dhelp = self.t("display_help");
        let user = self.username();
        let df = ui.form_field(
            c,
            left,
            &FieldSpec {
                label: &dlabel,
                tag: Some(&dtag),
                help: &dhelp,
                value: &p.display,
                placeholder: &user,
                caret: editing.then(|| p.dcaret.cursor(&p.display)),
                focus: self.focused(Row::Display),
                status: dstatus,
                w: fw,
                ..default()
            },
        );
        c.entity(df.input).insert(AccountAction::Edit(Editing::Display));
        // The avatar: the 32 looks, as large as the room beside allows.
        let aw = (list_w - fw - 3.0).max(16.0);
        let right = ui.boxed(c, two, UiStyle { w: Some(aw), gap: Some(0.6), shrink: Some(0.0), ..UiStyle::column() });
        ui.form_section(c, right, &self.t("avatar"), Some("32 looks"), Some(&self.t("avatar_help")));
        let a = p.avatar_now(self.fallback);
        let on = self.focused(Row::Avatar);
        let (images, icons) = (self.images.as_deref_mut(), self.icons.as_deref_mut());
        for (b, t) in ui.avatar_grid(c, right, images, icons, a, on, aw).into_iter().enumerate() {
            c.entity(t).insert(AccountAction::Avatar(b as u8));
        }
    }

    /// The right-hand tile: only what the player can't change here (the
    /// hub, its address masked, the role), then Link another device and
    /// Sign out.
    fn tile(&mut self, c: &mut Commands, cols: Entity, side_w: f32) {
        let ui = self.ui;
        let p = self.p;
        let tile = ui.boxed(c, cols, UiStyle { w: Some(side_w), gap: Some(0.6), pad: Some([1.2, 1.4].into()), shrink: Some(0.0), ..UiStyle::column() }.bg_a(Tone::Ink2, 0.75).outline(Tone::AccText, 0.22));
        ui.corners(c, tile, Tone::AccText, 1.0);
        let head = ui.boxed(c, tile, UiStyle { justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() });
        ui.text(c, head, Role::Mono, 0.54, Tone::AccText, &self.t("tile_title"));
        ui.text(c, head, Role::Mono, 0.5, Tone::Fg3, &self.t("read_only"));
        let who = ui.boxed(c, tile, UiStyle { gap: Some(1.0), align: Some(AlignItems::Center), pad: Some([0.4, 0.0].into()), ..UiStyle::row() });
        let a = p.avatar_now(self.fallback);
        let av = self.avatar(c, who, a, 3.0);
        c.entity(av).insert(Pickable::IGNORE);
        let names = ui.boxed(c, who, UiStyle { gap: Some(0.3), ..UiStyle::column() });
        ui.text_plain(c, names, Role::DisplayBold, 1.0, Tone::Fg, &self.display());
        ui.text_plain(c, names, Role::Mono, 0.56, Tone::Fg3, &format!("@{} · {}", self.username(), self.role()));
        let kw = 7.0;
        ui.value_row(c, tile, "Hub", RowValue::Text(self.hub), kw, true);
        let revealed = self.reveals.shown(REVEAL_HOST);
        ui.value_row(c, tile, "Address", RowValue::Secret { key: REVEAL_HOST, value: self.host, revealed, copied: self.copies.copied(REVEAL_HOST) }, kw, true);
        let role = self.role();
        ui.value_row(c, tile, "Role", RowValue::Badge(&role, BadgeLook::OUTLINE), kw, true);
        let status = if self.offline { "Offline" } else { "Online" };
        ui.value_row(c, tile, "Status", RowValue::Text(status), kw, true);
        if let Some(me) = p.me.as_ref().filter(|m| !m.previous_usernames.is_empty()) {
            let was = me.previous_usernames.iter().take(2).map(|u| format!("@{u}")).collect::<Vec<_>>().join(" ");
            ui.value_row(c, tile, "Was", RowValue::Mono(&was), kw, true);
        }
        // Link another device, Sign out.
        let btns = ui.boxed(c, tile, UiStyle { gap: Some(0.7), margin: Some([0.8, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        let lk = ui.button(c, btns, Kind::Secondary, &self.t(if p.link.is_some() { "link_again" } else { "link" }), None, Size::Regular, State { focus: self.focused(Row::Link), disabled: p.saving, ..State::NONE });
        c.entity(lk).insert(AccountAction::Link);
        if p.confirm_sign_out {
            let stay = ui.button(c, btns, Kind::Secondary, &self.t("stay"), Some("Esc"), Size::Regular, State::NONE);
            c.entity(stay).insert(AccountAction::SignOutStay);
            let go = ui.button(c, btns, Kind::DestructiveConfirm, &self.t("sign_out_confirm"), None, Size::Regular, State { focus: true, ..State::NONE });
            c.entity(go).insert(AccountAction::SignOutConfirm);
            let ask = ui.boxed(c, tile, UiStyle { w: Some(side_w - 2.8), gap: Some(0.6), pad: Some([0.6, 0.8].into()), ..UiStyle::row() }.bg_a(Tone::Threat, 0.07));
            ui.mark(c, ask, Tone::Threat, 0.45, 0.45);
            let words = ui.boxed(c, ask, UiStyle { grow: Some(1.0), shrink: Some(1.0), ..UiStyle::column() });
            let line = self.w.f("account.sign_out_ask", &[("user", &self.username()), ("hub", self.hub)]);
            ui.paragraph(c, words, Role::Text, 0.74, Tone::Fg, &line);
        } else {
            let b = ui.button(c, btns, Kind::Destructive, &self.t("sign_out_button"), None, Size::Regular, State { focus: self.focused(Row::SignOut), disabled: p.saving, ..State::NONE });
            c.entity(b).insert(AccountAction::SignOut);
        }
        if let Some((code, minutes)) = &p.link {
            let cells = ui.boxed(c, tile, UiStyle { gap: Some(0.3), align: Some(AlignItems::Center), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
            for (k, ch) in code.chars().enumerate() {
                if ch == '-' {
                    ui.text_plain(c, cells, Role::Mono, 1.0, Tone::Fg3, "–");
                    continue;
                }
                let cell = ui.boxed(c, cells, UiStyle { w: Some(1.9), h: Some(2.4), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), ..UiStyle::row() }.bg_a(Tone::Fg, 0.05).outline(if k == 0 { Tone::Acc } else { Tone::Fg4 }, 1.0));
                ui.text_plain(c, cell, Role::Mono, 1.2, Tone::Fg, &ch.to_string());
            }
            ui.text_plain(c, tile, Role::Mono, 0.52, Tone::AccText, &self.w.f("account.link_valid", &[("n", &minutes.to_string())]).to_uppercase());
            let w = ui.boxed(c, tile, UiStyle { w: Some(side_w - 2.8), ..UiStyle::column() });
            ui.paragraph(c, w, Role::Text, 0.72, Tone::Fg3, &self.t("link_where"));
        }
    }

    /// The Profile's action bar: its changes (one line however many),
    /// Discard and Save changes; all waiting while it saves.
    fn profile_footer(&mut self, c: &mut Commands, bar: Entity) {
        let ui = self.ui;
        let p = self.p;
        let changes = p.changes();
        let line = self.w.f("account.line", &[("user", &self.username()), ("hub", self.hub)]);
        let said = match &p.said {
            Some(s) => Some((if s.error { Tone::Threat } else { Tone::Good }, s.text.as_str())),
            None => Some((Tone::Fg4, line.as_str())),
        };
        let busy = self.t("saving_line");
        let (discard, save, saving) = (self.t("discard"), self.t("save"), self.t("saving"));
        let none = changes.is_empty();
        let actions = [
            FooterAction { kind: Kind::Tertiary, label: &discard, busy_label: None, key: None, disabled: none },
            FooterAction { kind: Kind::Primary, label: &save, busy_label: Some(&saving), key: Some("Ctrl+S"), disabled: none },
        ];
        let parts = ui.form_footer(c, bar, &FooterSpec { changes: &changes, open: p.changes_open, said, busy: p.saving.then_some(busy.as_str()), actions: &actions });
        if let Some(t) = parts.toggle {
            c.entity(t).insert(AccountAction::ToggleChanges);
        }
        c.entity(parts.actions[0]).insert(AccountAction::Discard);
        c.entity(parts.actions[1]).insert(AccountAction::Save);
    }

    /// Invites: the form (or, once made, the result card) at the left, the
    /// open invites beside it.
    fn invites(&mut self, c: &mut Commands, list: Entity, win_w: f32) {
        let ui = self.ui;
        let p = self.p;
        let fw = ui.theme.layout_opt("account", "invite_w").unwrap_or(30.0);
        let two = ui.boxed(c, list, UiStyle { gap: Some(2.6), align: Some(AlignItems::FlexStart), pad: Some([0.6, 0.0, 0.0, 0.4].into()), ..UiStyle::row() });
        let left = ui.boxed(c, two, UiStyle { w: Some(fw), gap: Some(0.8), shrink: Some(0.0), ..UiStyle::column() });
        match &p.made {
            Some(m) => {
                let link = m.link.clone().unwrap_or_else(|| m.code.clone());
                let title = self.w.f("account.made_title", &[("name", &m.name)]);
                let facts = [("Expires", day_month(m.expires)), ("Code", m.code.clone())];
                let (head, tag, copy, copied, another) = (self.t("made_head"), self.t("shown_once"), self.t("copy"), self.t("copied_short"), self.t("another"));
                let r = ui.result_card(
                    c,
                    left,
                    &ResultSpec { head: &head, tag: &tag, title: &title, value: &link, copy: &copy, copied: &copied, is_copied: self.copies.copied(COPY_INVITE), another: &another, facts: &facts },
                    fw,
                );
                c.entity(r.copy).insert(AccountAction::CopyLink);
                c.entity(r.another).insert(AccountAction::Another);
                let n = ui.boxed(c, left, UiStyle { w: Some(fw), ..UiStyle::column() });
                ui.paragraph(c, n, Role::Text, 0.72, Tone::Fg3, &self.t("not_kept"));
            }
            None => {
                ui.form_section(c, left, &self.t("make_section"), None, Some(&self.t("make_sentence")));
                for (row, e, text, caret, label, help, tag, ph) in [
                    (Row::Intent, Editing::Intent, &p.intent, p.icaret, "intent", "help.intent", "required", "intent_placeholder"),
                    (Row::Recipient, Editing::Recipient, &p.recipient, p.rcaret, "recipient", "help.recipient", "optional", "recipient_placeholder"),
                ] {
                    let editing = p.editing == Some(e);
                    let (label, help, tag, ph) = (self.t(label), self.t(help), self.t(tag), self.t(ph));
                    let needed = self.t("intent_needed");
                    let status = if row == Row::Intent && p.said.as_ref().is_some_and(|s| s.error) && text.trim().is_empty() {
                        FieldStatus::Error { text: &needed, fixes: &[] }
                    } else {
                        FieldStatus::None
                    };
                    let ff = ui.form_field(
                        c,
                        left,
                        &FieldSpec {
                            label: &label,
                            tag: Some(&tag),
                            help: &help,
                            value: text,
                            placeholder: &ph,
                            caret: editing.then(|| caret.cursor(text)),
                            focus: self.focused(row),
                            status,
                            w: fw,
                            ..default()
                        },
                    );
                    c.entity(ff.input).insert(AccountAction::Edit(e));
                }
                let options = [self.t("role_tester"), self.t("role_host")];
                let refs: Vec<&str> = options.iter().map(String::as_str).collect();
                let (label, help) = (self.t("role"), self.t("role_help"));
                let (_, ch) = ui.choice_field(c, left, &label, &help, &refs, p.role, None, State::focused(self.focused(Row::Role)), fw);
                if let Choice::Segmented(segs) = ch {
                    for (k, e) in segs.into_iter().enumerate() {
                        c.entity(e).insert(AccountAction::Role(k));
                    }
                }
                let making = p.pending.is_some() && p.made.is_none();
                let label = if making { self.t("making") } else { self.t("make") };
                let b = ui.button(c, left, Kind::Primary, &label, (!making).then_some("Enter"), Size::Regular, State { focus: self.focused(Row::Make), busy: making, ..State::NONE });
                c.entity(b).insert(AccountAction::Make);
            }
        }
        // The open invites, newest first.
        let iw = (win_w - fw - 4.0).max(30.0);
        let right = ui.boxed(c, two, UiStyle { w: Some(iw), gap: Some(0.6), shrink: Some(0.0), ..UiStyle::column() });
        let n = p.invites.as_ref().map_or(0, |l| l.len());
        ui.form_section(c, right, &self.t("open_section"), Some(&n.to_string()), Some(&self.t("open_sentence")));
        if n == 0 {
            let none = ui.boxed(c, right, UiStyle { w: Some(iw), ..UiStyle::column() });
            ui.paragraph(c, none, Role::Text, 0.78, Tone::Fg3, &self.t("open_none"));
        }
        let rows = p.rows();
        for inv in p.invites.iter().flatten() {
            let i = rows.iter().position(|r| *r == Row::Open(inv.id)).unwrap_or(0);
            let focus = p.focus == i;
            let asking = p.confirm_revoke == Some(inv.id);
            let style = UiStyle { w: Some(iw), gap: Some(0.9), align: Some(AlignItems::Center), pad: Some([0.5, 0.6, 0.5, 1.1].into()), ..UiStyle::row() };
            let style = if asking { style.bg_a(Tone::Threat, 0.07) } else { style };
            let item = ui.item(c, right, style, State::focused(focus && !asking), false);
            c.entity(item).insert((controls::MinHeight(3.4), AccountAction::Row(i)));
            if asking {
                ui.boxed(c, item, UiStyle { w: Some(0.24), ..UiStyle::row() }.abs([Some(0.0), Some(0.0), None, Some(0.0)]).bg(Tone::Threat));
                let q = ui.boxed(c, item, UiStyle { grow: Some(1.0), shrink: Some(1.0), gap: Some(0.2), ..UiStyle::column() });
                ui.text_plain(c, q, Role::DisplayBold, 0.78, Tone::Fg, &self.w.f("account.revoke_ask", &[("name", &inv.name)]).to_uppercase());
                ui.paragraph(c, q, Role::Text, 0.7, Tone::Fg2, &self.t("revoke_why"));
                let keep = ui.button(c, item, Kind::Secondary, &self.t("keep"), Some("Esc"), Size::Small, State::NONE);
                c.entity(keep).insert(AccountAction::RevokeKeep);
                let go = ui.button(c, item, Kind::DestructiveConfirm, &self.t("revoke_confirm"), None, Size::Small, State { focus: true, busy: p.pending.is_some(), ..State::NONE });
                c.entity(go).insert(AccountAction::RevokeConfirm(inv.id));
                continue;
            }
            let a = ui.boxed(c, item, UiStyle { grow: Some(1.0), shrink: Some(1.0), gap: Some(0.3), ..UiStyle::column() });
            ui.text_plain(c, a, Role::Text, 0.82, Tone::Fg, inv.intent.as_deref().unwrap_or(&inv.name));
            let sub = match inv.recipient.as_deref().filter(|r| !r.is_empty()) {
                Some(r) => format!("For {r} · made {}", day_month(inv.created)),
                None => format!("No recipient · made {}", day_month(inv.created)),
            };
            ui.text_plain(c, a, Role::Mono, 0.5, Tone::Fg3, &sub.to_uppercase());
            let role = if inv.role == "host" { "Host" } else { "Tester" };
            let look = if inv.role == "host" { BadgeLook::HERO } else { BadgeLook::OUTLINE };
            ui.badge_look(c, item, role, look);
            let e = ui.boxed(c, item, UiStyle { w: Some(6.4), shrink: Some(0.0), ..UiStyle::row() });
            ui.text_plain(c, e, Role::Mono, 0.54, Tone::Fg2, &format!("Until {}", day_month(inv.expires)).to_uppercase());
            let b = ui.button(c, item, Kind::Destructive, &self.t("revoke"), None, Size::Small, State { disabled: p.pending.is_some(), ..State::NONE });
            c.entity(b).insert(AccountAction::Revoke(inv.id));
        }
    }

    /// The Invites' action bar: what the last call did, else the count.
    fn invites_footer(&mut self, c: &mut Commands, bar: Entity) {
        let p = self.p;
        let n = p.invites.as_ref().map_or(0, |l| l.len());
        let line = self.w.f("account.open_line", &[("n", &n.to_string())]);
        let said = match &p.said {
            Some(s) => (if s.error { Tone::Threat } else { Tone::Good }, s.text.as_str()),
            None => (Tone::Fg4, line.as_str()),
        };
        let busy = self.t("asking");
        self.ui.form_footer(c, bar, &FooterSpec { said: Some(said), busy: (p.pending.is_some() && p.made.is_none()).then_some(busy.as_str()), ..default() });
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
        "display" => {
            p.me = staged.me().ok();
            p.focus = 1;
            p.editing = Some(Editing::Display);
            p.display = "Rocket \u{200b}Sam".into();
            p.dcaret = Caret::end_of(&p.display);
        }
        "avatar" => {
            p.focus = 2;
            p.avatar = Some(21);
        }
        "saving" => {
            p.me = staged.me().ok();
            p.display = "Night Owl".into();
            p.name = "night_owl".into();
            p.avatar = Some(21);
            p.saving = true;
            p.changes_open = false;
        }
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
                "ui_scene account {other}: profile, rename, rename-taken, display, avatar, saving, link, sign-out, invites, invite-made, player or off"
            ));
        }
    }
    Ok(())
}
