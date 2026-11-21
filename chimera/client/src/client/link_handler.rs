//! Invite links that open the game (docs/plans/active/onboarding.md,
//! decision 4): clicking a `chimera-hub://` link starts the game with the
//! link (or hands it to the game already running), and the app's first
//! run fills its field with it.
//!
//! - **The scheme** (`SCHEME`, `chimera_assets::account::SCHEME`) is
//!   registered per user, no admin rights (`SchemeRegistry`): on Windows
//!   under `HKCU\Software\Classes\chimera-hub` (`RegExe`, through
//!   `reg.exe`; its keys `reg_adds`), on Linux and the BSDs a desktop
//!   entry `chimera-hub.desktop` in `$XDG_DATA_HOME/applications` with
//!   `MimeType=x-scheme-handler/chimera-hub`, made the default in
//!   `$XDG_CONFIG_HOME/mimeapps.list` (`Xdg`: what `xdg-mime default`
//!   writes, without running it). The command is the running program with
//!   `+chimera:onboard <link>` (`OPEN_COMMAND`), its folder the working
//!   one. `ensure` writes only when the scheme opens something else (a
//!   first launch, the program moved, another app took it): once a run,
//!   a few frames in (the config read), on a thread, when `cl_link_handler`
//!   is 1 (default; 0 never writes), not in a `dev` build (unless
//!   `CHIMERA_LINK_HANDLER=1`), not with `CHIMERA_NO_LINK_HANDLER` set,
//!   and only in an app that set up `LinkSetup::user` (not a tool run or a
//!   test).
//! - **Untrusted input**: what a link brings is read by `accept` only: a
//!   well-formed `chimera-hub://<host>:<port>[/<fingerprint>]?code=<code>`
//!   (letters, digits, dots and dashes in the host, a port, an optional
//!   fingerprint, the code an invite's or a link code's and nothing else),
//!   given back rebuilt from its parts. A start by a link
//!   (`link_launch`: the first argument `+chimera:onboard`) takes nothing
//!   else from its command line, so nothing but an invite passes through
//!   the scheme (a link that breaks out of its quotes is one malformed
//!   link: the game starts without it).
//! - **One game at a time takes links** (`LinkInbox`, a folder,
//!   `<data_local>/chimera/links`): a running app with `LinkSetup` holds a
//!   lock on its own `running-<pid>.lock` there (`LinkInbox::hold`; the
//!   OS lets go when the process ends, however it ends). A start by a link
//!   that finds such a lock held (`running`) drops the link there as a
//!   file (`hand`, written then renamed) and exits; the running app looks
//!   every `POLL_SECS` (`take`: read, removed, checked again by `accept`,
//!   older than `MAX_AGE` dropped) and brings its first run up with it.
//!   Files, not a socket: nothing listens, nothing to bind, the same on
//!   Windows and Linux.
//! - `OpenedLinks`: the links to show, from the command line
//!   (`Args::opened_link`) or the inbox; the app's first run takes them
//!   (`party_kit::onboarding`).
//!
//! Known shortcuts: no macOS registration (not a target); unregistering is
//! by hand (`cl_link_handler 0` stops the writes, it removes nothing);
//! a link handed to a game that exits before it looks waits for the next
//! start (within `MAX_AGE`).

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use chimera_assets::{account::HubLink, codes};

/// The scheme's name (`chimera-hub://`).
pub const SCHEME: &str = "chimera-hub";
/// The command a link opens the game with: `+chimera:onboard <link>`.
pub const OPEN_COMMAND: &str = "chimera:onboard";
/// The desktop entry's file name (Linux).
pub const DESKTOP_FILE: &str = "chimera-hub.desktop";
/// The variable that stops the registration (agents' and CI's runs).
pub const NO_HANDLER_ENV: &str = "CHIMERA_NO_LINK_HANDLER";
/// The variable that allows it in a `dev` build.
pub const HANDLER_ENV: &str = "CHIMERA_LINK_HANDLER";
/// The longest link taken, characters.
pub const MAX_LINK: usize = 256;
/// Seconds between the running app's looks at its inbox.
pub const POLL_SECS: f64 = 0.5;
/// A link in the inbox older than this is dropped.
pub const MAX_AGE: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// What a link may bring.

/// `text` as an invite link (module doc, "Untrusted input"): the link
/// rebuilt from its checked parts, or None.
pub fn accept(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.len() > MAX_LINK || !t.bytes().all(|b| b.is_ascii_graphic()) {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let rest = lower.strip_prefix(chimera_assets::account::SCHEME)?;
    // The scheme in lower case, the rest as given.
    let t = format!("{}{}", chimera_assets::account::SCHEME, &t[t.len() - rest.len()..]);
    let (before, query) = t.split_once('?')?;
    let code = query.strip_prefix("code=")?;
    // The code and nothing after it: no other parameter, no fragment.
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    let n = codes::normalize(code)?;
    if n.len() != codes::INVITE_LEN && n.len() != codes::LINK_LEN {
        return None;
    }
    let link = HubLink::parse(&format!("{before}?code={code}")).ok()?;
    let (host, port) = link.host.rsplit_once(':')?;
    let host_ok = if let Some(v6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !v6.is_empty() && v6.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
    } else {
        !host.is_empty()
            && host.len() <= 253
            && !host.starts_with(['-', '.'])
            && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    };
    let port_ok = port.parse::<u16>().is_ok_and(|p| p > 0) && !port.starts_with('+');
    if !host_ok || !port_ok || link.code.as_deref() != Some(code) {
        return None;
    }
    Some(link.to_text(true))
}

/// A start by a link: the first argument is `+chimera:onboard` (what the
/// registration writes). `Some(Some(link))` when what follows is one
/// invite link (`accept`; the words joined, in case a quote broke it),
/// `Some(None)` when it isn't (start without it), None for any other
/// command line.
pub fn link_launch(args: &[String]) -> Option<Option<String>> {
    let first = args.first()?;
    let name = first.strip_prefix('+')?;
    if !name.eq_ignore_ascii_case(OPEN_COMMAND) {
        return None;
    }
    Some(accept(&args[1..].join(" ")))
}

// ---------------------------------------------------------------------------
// The registration.

/// What the scheme opens: this program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub exe: PathBuf,
    /// The app's name ("Ultimate Gun Game"), for the desktop entry.
    pub name: String,
}

/// Where the scheme is registered (module doc): the system's, or a fake
/// or temporary one in tests.
pub trait SchemeRegistry: Send + Sync {
    /// The command the scheme opens now, as this registry writes it; None
    /// when nothing or another program's entry has it.
    fn current(&self) -> Option<String>;
    /// The command it writes for `h`.
    fn command(&self, h: &Handler) -> String;
    fn register(&self, h: &Handler) -> Result<(), String>;
}

/// Registers `h` unless the scheme opens it already; true when it wrote.
pub fn ensure(reg: &dyn SchemeRegistry, h: &Handler) -> Result<bool, String> {
    if reg.current().as_deref() == Some(reg.command(h).as_str()) {
        return Ok(false);
    }
    reg.register(h)?;
    Ok(true)
}

/// The scheme's command on Windows: `"<exe>" +chimera:onboard "%1"`.
pub fn windows_command(exe: &Path) -> String {
    format!("\"{}\" +{OPEN_COMMAND} \"%1\"", exe.display())
}

/// The `reg.exe` calls that register `h` under HKCU (no admin rights).
pub fn reg_adds(h: &Handler) -> Vec<Vec<String>> {
    let root = format!(r"HKCU\Software\Classes\{SCHEME}");
    let add = |key: String, value: Option<&str>, data: String| {
        let mut v = vec!["add".to_string(), key];
        match value {
            Some(name) => v.extend(["/v".to_string(), name.to_string()]),
            None => v.push("/ve".into()),
        }
        v.extend(["/t".into(), "REG_SZ".into(), "/d".into(), data, "/f".into()]);
        v
    };
    vec![
        add(root.clone(), None, format!("URL:{} invite link", h.name)),
        add(root.clone(), Some("URL Protocol"), String::new()),
        add(format!(r"{root}\DefaultIcon"), None, format!("\"{}\",0", h.exe.display())),
        add(format!(r"{root}\shell\open\command"), None, windows_command(&h.exe)),
    ]
}

/// The query that reads the command back.
pub fn reg_query() -> Vec<String> {
    vec!["query".into(), format!(r"HKCU\Software\Classes\{SCHEME}\shell\open\command"), "/ve".into()]
}

/// The default value from `reg query ... /ve`'s output (the text after
/// `REG_SZ` on its line; the value's name is in the system's language).
pub fn parse_reg_query(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, v)| v.trim().to_string())).filter(|v| !v.is_empty())
}

/// Runs `reg.exe` with its arguments, its output back.
pub type RegRunner = Box<dyn Fn(&[String]) -> Result<String, String> + Send + Sync>;

/// Windows' registration through `reg.exe` (`run`: the system's, or a
/// fake in tests).
pub struct RegExe {
    pub run: RegRunner,
}

impl RegExe {
    /// The system's `reg.exe`, without a console window.
    pub fn system() -> Self {
        RegExe {
            run: Box::new(|args| {
                let mut cmd = std::process::Command::new("reg");
                cmd.args(args);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    // CREATE_NO_WINDOW.
                    cmd.creation_flags(0x0800_0000);
                }
                let out = cmd.output().map_err(|e| format!("reg: {e}"))?;
                if out.status.success() {
                    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
                } else {
                    Err(format!("reg {}: {}", args.first().map_or("", |s| s.as_str()), String::from_utf8_lossy(&out.stderr).trim()))
                }
            }),
        }
    }
}

impl SchemeRegistry for RegExe {
    fn current(&self) -> Option<String> {
        (self.run)(&reg_query()).ok().as_deref().and_then(parse_reg_query)
    }

    fn command(&self, h: &Handler) -> String {
        windows_command(&h.exe)
    }

    fn register(&self, h: &Handler) -> Result<(), String> {
        for args in reg_adds(h) {
            (self.run)(&args)?;
        }
        Ok(())
    }
}

/// One argument of a desktop entry's `Exec`, quoted (the desktop entry
/// spec: `"`, `` ` ``, `$` and `\` escaped inside quotes, `%` doubled),
/// then escaped as the file's string value (`\` doubled).
pub fn desktop_quote(arg: &str) -> String {
    let mut q = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                q.push('\\');
                q.push(c);
            }
            '%' => q.push_str("%%"),
            c => q.push(c),
        }
    }
    q.push('"');
    q.replace('\\', "\\\\")
}

/// The desktop entry's `Exec` for `exe`.
pub fn desktop_exec(exe: &Path) -> String {
    format!("{} +{OPEN_COMMAND} %u", desktop_quote(&exe.to_string_lossy()))
}

/// The desktop entry for `h`.
pub fn desktop_entry(h: &Handler) -> String {
    let name: String = h.name.chars().filter(|c| !c.is_control()).collect();
    let dir = h.exe.parent().map(|d| d.to_string_lossy().replace('\\', "\\\\")).unwrap_or_default();
    format!(
        "[Desktop Entry]\nType=Application\nName={name} invite link\nComment=Opens {name} with a hub invite\nExec={}\nPath={dir}\n\
         MimeType=x-scheme-handler/{SCHEME};\nNoDisplay=true\nTerminal=false\n",
        desktop_exec(&h.exe)
    )
}

/// `mimeapps.list` with `x-scheme-handler/<SCHEME>` defaulting to
/// `DESKTOP_FILE` under `[Default Applications]`, the rest kept.
pub fn with_default(list: &str) -> String {
    let key = format!("x-scheme-handler/{SCHEME}");
    let entry = format!("{key}={DESKTOP_FILE}");
    let mut out: Vec<String> = Vec::new();
    let mut in_defaults = false;
    let mut placed = false;
    for line in list.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if in_defaults && !placed {
                out.push(entry.clone());
                placed = true;
            }
            in_defaults = t == "[Default Applications]";
        } else if in_defaults && t.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            if !placed {
                out.push(entry.clone());
                placed = true;
            }
            continue;
        }
        out.push(line.to_string());
    }
    if in_defaults && !placed {
        out.push(entry.clone());
        placed = true;
    }
    if !placed {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push("[Default Applications]".into());
        out.push(entry);
    }
    out.join("\n") + "\n"
}

/// The default for the scheme in a `mimeapps.list`.
pub fn default_in(list: &str) -> Option<String> {
    let key = format!("x-scheme-handler/{SCHEME}");
    let mut in_defaults = false;
    for line in list.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_defaults = t == "[Default Applications]";
        } else if in_defaults && let Some((k, v)) = t.split_once('=')
            && k.trim() == key
        {
            return v.split(';').map(str::trim).find(|s| !s.is_empty()).map(String::from);
        }
    }
    None
}

/// The XDG registration (Linux, the BSDs): a desktop entry and the
/// default in `mimeapps.list`, under the given folders.
pub struct Xdg {
    /// `$XDG_DATA_HOME` (`~/.local/share`).
    pub data_home: PathBuf,
    /// `$XDG_CONFIG_HOME` (`~/.config`).
    pub config_home: PathBuf,
}

impl Xdg {
    /// The user's folders (`$XDG_DATA_HOME`, `$XDG_CONFIG_HOME` or their
    /// defaults).
    pub fn user() -> Option<Self> {
        Some(Xdg { data_home: dirs::data_dir()?, config_home: dirs::config_dir()? })
    }

    pub fn desktop_path(&self) -> PathBuf {
        self.data_home.join("applications").join(DESKTOP_FILE)
    }

    pub fn mimeapps_path(&self) -> PathBuf {
        self.config_home.join("mimeapps.list")
    }
}

impl SchemeRegistry for Xdg {
    fn current(&self) -> Option<String> {
        let list = fs::read_to_string(self.mimeapps_path()).ok()?;
        if default_in(&list)? != DESKTOP_FILE {
            return None;
        }
        let entry = fs::read_to_string(self.desktop_path()).ok()?;
        entry.lines().find_map(|l| l.strip_prefix("Exec=")).map(String::from)
    }

    fn command(&self, h: &Handler) -> String {
        desktop_exec(&h.exe)
    }

    fn register(&self, h: &Handler) -> Result<(), String> {
        let desktop = self.desktop_path();
        let say = |p: &Path, e: io::Error| format!("{}: {e}", p.display());
        fs::create_dir_all(desktop.parent().unwrap_or(&self.data_home)).map_err(|e| say(&desktop, e))?;
        write_replacing(&desktop, desktop_entry(h).as_bytes()).map_err(|e| say(&desktop, e))?;
        let list_path = self.mimeapps_path();
        let list = match fs::read_to_string(&list_path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(say(&list_path, e)),
        };
        fs::create_dir_all(&self.config_home).map_err(|e| say(&list_path, e))?;
        write_replacing(&list_path, with_default(&list).as_bytes()).map_err(|e| say(&list_path, e))
    }
}

/// Writes `path` through a file beside it, renamed over it.
fn write_replacing(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// This system's registry (module doc); None where there is none.
pub fn user_registry() -> Option<Arc<dyn SchemeRegistry>> {
    #[cfg(windows)]
    {
        Some(Arc::new(RegExe::system()))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Xdg::user().map(|x| Arc::new(x) as Arc<dyn SchemeRegistry>)
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    {
        None
    }
}

/// Whether this run may write the registration (module doc), `setting`
/// being `cl_link_handler`.
pub fn may_register(setting: u8) -> bool {
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0");
    setting != 0 && !set(NO_HANDLER_ENV) && (!cfg!(feature = "dev") || set(HANDLER_ENV))
}

// ---------------------------------------------------------------------------
// One game takes links.

/// The folder a link is handed to the running game through (module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkInbox {
    pub dir: PathBuf,
}

/// A running app's hold on the inbox (dropped: the lock file goes).
#[derive(Debug)]
pub struct InboxHold {
    file: Option<fs::File>,
    path: PathBuf,
}

impl Drop for InboxHold {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

impl LinkInbox {
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        LinkInbox { dir: dir.into() }
    }

    /// `<data_local>/chimera/links`.
    pub fn user() -> Option<Self> {
        Some(Self::at(dirs::data_local_dir()?.join("chimera").join("links")))
    }

    /// Holds the inbox for this process (`running` sees it until the
    /// hold drops or the process ends).
    pub fn hold(&self) -> io::Result<InboxHold> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(format!("running-{}.lock", std::process::id()));
        let file = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path)?;
        file.try_lock().map_err(|e| match e {
            fs::TryLockError::WouldBlock => io::Error::new(io::ErrorKind::WouldBlock, "held already"),
            fs::TryLockError::Error(e) => e,
        })?;
        Ok(InboxHold { file: Some(file), path })
    }

    /// Whether a running app holds the inbox; lock files nobody holds
    /// (a crashed app's) are removed on the way.
    pub fn running(&self) -> bool {
        let Ok(dir) = fs::read_dir(&self.dir) else { return false };
        let mut any = false;
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("running-") && name.ends_with(".lock")) {
                continue;
            }
            let Ok(file) = fs::OpenOptions::new().read(true).write(true).open(entry.path()) else { continue };
            match file.try_lock() {
                Ok(()) => {
                    drop(file);
                    let _ = fs::remove_file(entry.path());
                }
                Err(fs::TryLockError::WouldBlock) => any = true,
                Err(fs::TryLockError::Error(_)) => {}
            }
        }
        any
    }

    /// Hands `link` to the running app (a file written beside, renamed in).
    pub fn hand(&self, link: &str) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.dir)?;
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let stem = format!("{nanos:032}-{}", std::process::id());
        let tmp = self.dir.join(format!("{stem}.tmp"));
        let path = self.dir.join(format!("{stem}.link"));
        fs::write(&tmp, link)?;
        fs::rename(&tmp, &path)?;
        Ok(path)
    }

    /// The links handed in, oldest first, each removed and checked again
    /// (`accept`); older than `MAX_AGE`, or not a link, dropped.
    pub fn take(&self) -> Vec<String> {
        let Ok(dir) = fs::read_dir(&self.dir) else { return Vec::new() };
        let mut files: Vec<PathBuf> =
            dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "link")).collect();
        files.sort();
        let mut out = Vec::new();
        for p in files {
            let fresh = fs::metadata(&p)
                .ok()
                .filter(|m| m.len() <= MAX_LINK as u64 * 2)
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t.elapsed().map_or(true, |age| age <= MAX_AGE));
            let text = fresh.then(|| fs::read_to_string(&p).ok()).flatten();
            // Another app took it first: not ours.
            if fs::remove_file(&p).is_err() {
                continue;
            }
            if let Some(link) = text.as_deref().and_then(accept) {
                out.push(link);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The app's side.

/// Invite links to show (module doc): taken by the app's first run.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenedLinks(pub Vec<String>);

/// `cl_link_handler`: 1 (default) registers the scheme for this program,
/// 0 never writes it.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkHandlerSetting(pub u8);

impl Default for LinkHandlerSetting {
    fn default() -> Self {
        LinkHandlerSetting(1)
    }
}

/// The app's registration and inbox (module doc); only a real app's start
/// inserts one (`user`), so tools and tests write nothing.
#[derive(Resource)]
pub struct LinkSetup {
    pub registry: Option<Arc<dyn SchemeRegistry>>,
    pub handler: Option<Handler>,
    pub inbox: Option<LinkInbox>,
    /// The inbox held while the app runs.
    pub hold: Option<InboxHold>,
    /// Set once the registration has been looked at.
    pub registered: bool,
}

impl LinkSetup {
    /// The user's (the system's registry, this program, the user's inbox,
    /// held now).
    pub fn user(name: &str) -> Self {
        let inbox = LinkInbox::user();
        let hold = inbox.as_ref().and_then(|i| i.hold().inspect_err(|e| warn!("link inbox: {e}")).ok());
        LinkSetup {
            registry: user_registry(),
            handler: std::env::current_exe().ok().map(|exe| Handler { exe, name: name.to_string() }),
            inbox,
            hold,
            registered: false,
        }
    }
}

pub struct LinkHandlerPlugin;

impl Plugin for LinkHandlerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpenedLinks>().init_resource::<LinkHandlerSetting>();
        crate::console::resource_cvar::<LinkHandlerSetting, u8>(
            app,
            "cl_link_handler",
            "1 (default): invite links (chimera-hub://) open this game; registered for this user when the game starts. 0: never registered.",
            |s| &mut s.0,
        );
        {
            let mut console = app.world_mut().resource_mut::<chimera_core::console::Console>();
            console.archive("cl_link_handler");
            console.set_range("cl_link_handler", 0.0, 1.0);
        }
        app.add_systems(Update, (register_once, poll_inbox).run_if(resource_exists::<LinkSetup>));
    }
}

/// The registration, once, a few frames in (the config read by then), on
/// a thread.
fn register_once(mut setup: ResMut<LinkSetup>, setting: Res<LinkHandlerSetting>, mut frames: Local<u32>) {
    *frames += 1;
    if setup.registered || *frames < 4 {
        return;
    }
    setup.registered = true;
    if !may_register(setting.0) {
        return;
    }
    let (Some(reg), Some(h)) = (setup.registry.clone(), setup.handler.clone()) else { return };
    std::thread::spawn(move || match ensure(reg.as_ref(), &h) {
        Ok(true) => info!("{SCHEME}:// links open {}", h.exe.display()),
        Ok(false) => {}
        Err(e) => warn!("{SCHEME}:// links: {e}"),
    });
}

/// The inbox, every `POLL_SECS`, into `OpenedLinks`.
fn poll_inbox(setup: Res<LinkSetup>, time: Res<Time<Real>>, mut last: Local<Option<f64>>, mut links: ResMut<OpenedLinks>) {
    let now = time.elapsed_secs_f64();
    if last.is_some_and(|l| now - l < POLL_SECS) {
        return;
    }
    *last = Some(now);
    if let Some(inbox) = &setup.inbox {
        let taken = inbox.take();
        if !taken.is_empty() {
            links.0.extend(taken);
        }
    }
}
