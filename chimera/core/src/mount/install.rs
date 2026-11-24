//! Finding a game's install, in this order:
//!
//! 0. `$CHIMERA_<GAME>_ROOT` (`root_env`: `CHIMERA_HL2DM_ROOT`,
//!    `CHIMERA_CS_SOURCE_ROOT`), for one run: a SteamCMD install outside
//!    Steam's libraries, or a test against another copy, without editing
//!    the config;
//! 1. its `path` in `chimera.local.toml` (`config::LocalConfig`: an
//!    explicit path always wins, even one that doesn't check out);
//! 2. the path saved in the user's settings file (`settings_path`: the
//!    first-run dialog and `chimera_install` write it), while it still
//!    checks out;
//! 3. Steam: each Steam root (`steam_roots`; `$CHIMERA_STEAM_ROOT`
//!    replaces the usual places), its libraries
//!    (`steamapps/libraryfolders.vdf`, old and new formats), the library
//!    with the app's `appmanifest_<id>.acf` (fully installed), its
//!    `steamapps/common/<installdir>` holding the files the game needs;
//! 4. the games folder convention (`Places`, `find_by_convention`): a
//!    folder of the game's name (`SteamGame::folders`: `cs16`,
//!    `counter-strike-source`) in `~/games` (Windows `%USERPROFILE%\games`;
//!    `$CHIMERA_GAMES_DIR` replaces it), holding the files the game needs;
//! 5. a dedicated server, only when no client was found: SteamCMD's
//!    default places (`steamcmd_roots`: `~/steamcmd`, `~/Steam`, ... a
//!    Steam root of their own, searched for the game's
//!    `SteamGame::server_apps`), then the games folder's server folders
//!    (`cs16_ds`, `cs16-ds`, `cs16_server`, ...: the folder's name and
//!    `ds`, `server` or `dedicated`).
//!
//! The client comes before a dedicated server in every place because it
//! has more content. The mount doctor and `Resolved::describe` say which
//! rule found the install (`Found`).
//!
//! The Steam build id comes with it (from the manifest) for the mount
//! doctor. Only a game that names its Steam app (`GameInfo::steam`, a
//! `SteamGame`: app id, required files) is looked for on Steam; this layer
//! takes that as an argument and stays below the game modules.

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use super::{
    config::LocalConfig,
    keyvalues::{self, Escapes, Kv},
};

/// A game Steam installs: its Steam app and the files (relative to the
/// install folder) that show it is that game. A game says it in
/// `GameInfo::steam`.
#[derive(Clone, Copy, Debug)]
pub struct SteamGame {
    pub app_id: u32,
    pub name: &'static str,
    pub required: &'static [&'static str],
    /// The game's dedicated server apps (SteamCMD, anonymous), looked for
    /// in SteamCMD's default places after the client. Empty: none.
    pub server_apps: &'static [u32],
    /// Folder names the game goes by in a games folder
    /// (`~/games/<name>`): `cs16`, `counter-strike-source`. Case, `_`, `-`
    /// and spaces don't matter, and `<name>_ds`, `<name>-server` and the
    /// like name a dedicated server. Empty: no convention.
    pub folders: &'static [&'static str],
}

/// Replaces the usual Steam roots (paths joined as `PATH` is; empty: no
/// Steam at all), for tests and for trying the first-run dialog.
pub const STEAM_ROOT_ENV: &str = "CHIMERA_STEAM_ROOT";
/// Replaces the user's settings file.
pub const SETTINGS_ENV: &str = "CHIMERA_SETTINGS";
/// Replaces the games folders (`~/games`; paths joined as `PATH` is;
/// empty: none), where `SteamGame::folders` are looked for.
pub const GAMES_DIR_ENV: &str = "CHIMERA_GAMES_DIR";

/// The environment variable naming `game`'s install for this run:
/// `CHIMERA_<GAME>_ROOT`, the id upper-cased (`CHIMERA_HL2DM_ROOT`).
pub fn root_env(game: &str) -> String {
    format!("CHIMERA_{}_ROOT", game.to_ascii_uppercase())
}

/// How an install was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The environment variable (`root_env`).
    Env(String),
    /// `chimera.local.toml` (the file, when known).
    Config(Option<PathBuf>),
    /// The user's settings file.
    Saved(PathBuf),
    /// Steam: the library's number in `root`'s `libraryfolders.vdf` (0:
    /// the Steam folder itself) and its folder.
    Steam { root: PathBuf, library: usize, library_path: PathBuf },
    /// A dedicated server in a SteamCMD default place (`steamcmd_roots`):
    /// its app, the root and the library as for `Steam`.
    SteamCmd { app_id: u32, root: PathBuf, library: usize, library_path: PathBuf },
    /// The games folder convention: the folder `name` in `games_dir`; a
    /// `server` one is a dedicated server's (`<name>_ds`).
    Convention { games_dir: PathBuf, name: String, server: bool },
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Found::Env(var) => write!(f, "${var}"),
            Found::Config(Some(file)) => write!(f, "config {}", file.display()),
            Found::Config(None) => f.write_str("config"),
            Found::Saved(file) => write!(f, "saved settings {}", file.display()),
            Found::Steam { library, library_path, .. } => {
                write!(f, "Steam library {library} ({})", library_path.display())
            }
            Found::SteamCmd { app_id, library_path, .. } => {
                write!(f, "SteamCMD default place, dedicated server app {app_id} ({})", library_path.display())
            }
            Found::Convention { games_dir, name, server } => write!(
                f,
                "games folder convention, {} {}",
                if *server { "dedicated server" } else { "client" },
                games_dir.join(name).display()
            ),
        }
    }
}

/// A game's install folder, how it was found, its Steam build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    pub path: PathBuf,
    pub found: Found,
    pub build_id: Option<String>,
}

/// The outcome of a search: the install, and what was tried on the way
/// (paths that didn't check out, libraries without the app).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    pub install: Option<Install>,
    pub notes: Vec<String>,
}

impl Resolved {
    /// One line for the log and the console.
    pub fn describe(&self, name: &str) -> String {
        match &self.install {
            Some(i) => {
                let build = i.build_id.as_deref().map(|b| format!(", build {b}")).unwrap_or_default();
                format!("{name} install: {} (from {}{build})", i.path.display(), i.found)
            }
            None if self.notes.is_empty() => format!("{name} install: none found"),
            None => format!("{name} install: none found ({})", self.notes.join("; ")),
        }
    }
}

// ---------------------------------------------------------------------------
// Checking a folder.

/// Whether `path` holds `game`'s files: Ok, or what's missing.
pub fn check(path: &Path, game: &SteamGame) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("{}: not a folder", path.display()));
    }
    let missing: Vec<&str> = game.required.iter().copied().filter(|f| !path.join(f).is_file()).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("{} has no {}: not a {} folder", path.display(), missing.join(" or "), game.name))
    }
}

/// `~` at the start means the home folder.
pub fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => dirs::home_dir().map_or_else(|| path.to_path_buf(), |h| h.join(rest)),
        Err(_) => path.to_path_buf(),
    }
}

// ---------------------------------------------------------------------------
// Steam.

/// A Steam app manifest's facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub install_dir: String,
    pub build_id: Option<String>,
    pub state_flags: u32,
}

/// `StateFlags` bit: fully installed.
const FULLY_INSTALLED: u32 = 4;

impl Manifest {
    pub fn fully_installed(&self) -> bool {
        self.state_flags & FULLY_INSTALLED != 0
    }
}

/// Read an `appmanifest_<id>.acf`.
pub fn read_manifest(path: &Path) -> Option<Manifest> {
    let text = fs::read_to_string(path).ok()?;
    let root = keyvalues::parse(&text, Escapes::On);
    let state = root.get("AppState")?;
    Some(Manifest {
        install_dir: state.str("installdir")?.to_string(),
        build_id: state.str("buildid").map(String::from),
        state_flags: state.str("StateFlags").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
    })
}

/// A Steam library: its number in `libraryfolders.vdf`, its folder, the
/// apps it lists (none in the old format).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    pub index: usize,
    pub path: PathBuf,
    pub apps: Vec<u32>,
}

/// The libraries of a Steam root: the root itself (library 0) and those
/// `steamapps/libraryfolders.vdf` names. Both formats: the current one
/// (`"1" { "path" "..." "apps" { "240" "..." } }`) and the old one
/// (`"1" "D:\\SteamLibrary"`).
pub fn libraries(root: &Path) -> Vec<Library> {
    let mut out: Vec<Library> = Vec::new();
    let text = ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"]
        .iter()
        .find_map(|f| fs::read_to_string(root.join(f)).ok())
        .unwrap_or_default();
    let kv = keyvalues::parse(&text, Escapes::On);
    let top = kv.items().first().map(|(_, v)| v);
    for (key, node) in top.map(Kv::items).unwrap_or(&[]) {
        let Ok(index) = key.trim().parse::<usize>() else {
            continue;
        };
        let (path, apps) = match node {
            Kv::Value(p) => (p.clone(), Vec::new()),
            Kv::Block(_) => {
                let Some(p) = node.str("path") else { continue };
                let apps = node
                    .get("apps")
                    .map_or(Vec::new(), |a| a.items().iter().filter_map(|(k, _)| k.trim().parse().ok()).collect());
                (p.to_string(), apps)
            }
        };
        out.push(Library { index, path: PathBuf::from(path), apps });
    }
    if !out.iter().any(|l| same_dir(&l.path, root)) {
        out.insert(0, Library { index: 0, path: root.to_path_buf(), apps: Vec::new() });
    }
    out
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// Look for `game` in the libraries of `roots`.
pub fn find_in_steam(roots: &[PathBuf], game: &SteamGame) -> Resolved {
    let mut notes = Vec::new();
    let manifest_name = format!("appmanifest_{}.acf", game.app_id);
    for root in roots {
        let mut libs = libraries(root);
        // Libraries that list the app first; the manifests decide.
        libs.sort_by_key(|l| !l.apps.contains(&game.app_id));
        for lib in libs {
            let listed = lib.apps.contains(&game.app_id);
            let steamapps = lib.path.join("steamapps");
            let Some(manifest) = read_manifest(&steamapps.join(&manifest_name)) else {
                if listed {
                    notes.push(format!(
                        "Steam library {} lists app {} but has no {manifest_name}",
                        lib.index, game.app_id
                    ));
                }
                continue;
            };
            if !manifest.fully_installed() {
                notes.push(format!(
                    "Steam library {}: {} isn't fully installed (StateFlags {})",
                    lib.index, game.name, manifest.state_flags
                ));
                continue;
            }
            let path = steamapps.join("common").join(&manifest.install_dir);
            if let Err(e) = check(&path, game) {
                notes.push(format!("Steam library {}: {e}", lib.index));
                continue;
            }
            return Resolved {
                install: Some(Install {
                    path,
                    found: Found::Steam { root: root.clone(), library: lib.index, library_path: lib.path.clone() },
                    build_id: manifest.build_id,
                }),
                notes,
            };
        }
    }
    if roots.is_empty() {
        notes.push("no Steam folder found".into());
    } else if notes.is_empty() {
        notes.push(format!("no Steam library has {} (app {})", game.name, game.app_id));
    }
    Resolved { install: None, notes }
}

/// The folders a game is looked for in: Steam's (client installs), SteamCMD's
/// default places (dedicated servers), the games folders (conventions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places {
    pub steam: Vec<PathBuf>,
    pub steamcmd: Vec<PathBuf>,
    pub games: Vec<PathBuf>,
}

impl Places {
    /// This system's: `steam_roots`, `steamcmd_roots`, `games_dirs`.
    pub fn system() -> Self {
        Self { steam: steam_roots(), steamcmd: steamcmd_roots(), games: games_dirs() }
    }
}

/// Existing folders of `candidates`, each once.
fn existing(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen = Vec::new();
    for c in candidates {
        let Ok(canon) = fs::canonicalize(&c) else { continue };
        if canon.is_dir() && !seen.contains(&canon) {
            seen.push(canon);
            out.push(c);
        }
    }
    out
}

/// Whether the environment says where Steam is (`$CHIMERA_STEAM_ROOT`,
/// even empty): a test or a trial of the first-run dialog, which then
/// looks nowhere else unless `$CHIMERA_GAMES_DIR` says so.
fn steam_pinned() -> bool {
    crate::env::var_os(STEAM_ROOT_ENV).is_some()
}

/// SteamCMD's default places, existing ones: the folders it installs
/// into without `force_install_dir` (`<folder>/steamapps/common/...`):
/// `~/steamcmd`, `~/Steam`, `~/.steam/steamcmd`, `~/.local/share/Steam`,
/// on Windows `%USERPROFILE%\steamcmd` and `C:\steamcmd`. Each is read as
/// a Steam root (its libraries, its manifests). None while
/// `$CHIMERA_STEAM_ROOT` is set.
pub fn steamcmd_roots() -> Vec<PathBuf> {
    if steam_pinned() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        for d in ["steamcmd", "SteamCMD", "Steam", ".steam/steamcmd", ".local/share/Steam"] {
            out.push(home.join(d));
        }
    }
    #[cfg(windows)]
    {
        out.push(PathBuf::from(r"C:\steamcmd"));
        out.push(PathBuf::from(r"C:\SteamCMD"));
    }
    existing(out)
}

/// The games folders, existing ones: `$CHIMERA_GAMES_DIR` (paths joined
/// as `PATH` is) replaces `~/games` and `~/Games` (Windows:
/// `%USERPROFILE%\games`). None while `$CHIMERA_STEAM_ROOT` is set
/// without it.
pub fn games_dirs() -> Vec<PathBuf> {
    if let Some(v) = crate::env::var_os(GAMES_DIR_ENV) {
        return existing(std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()));
    }
    if steam_pinned() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("games"));
        out.push(home.join("Games"));
    }
    existing(out)
}

/// A folder name without case and separators: `Counter-Strike_Source`
/// and `counterstrikesource` are the same.
fn flat(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// What a name's last part says of the folder: a dedicated server's.
const SERVER_SUFFIXES: &[&str] = &["ds", "server", "dedicated", "dedicatedserver", "srv"];

/// Look in each games folder for folders named for `game`: the client's
/// (`server` false: `<hint>`) or a dedicated server's (`<hint>_ds`,
/// `<hint>-ds`, `<hint>_server`, ...), the first whose files check out.
pub fn find_by_convention(games_dirs: &[PathBuf], game: &SteamGame, server: bool) -> Resolved {
    let mut notes = Vec::new();
    let hints: Vec<String> = game.folders.iter().map(|h| flat(h)).collect();
    for dir in games_dirs {
        let mut names: Vec<String> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        // Hints' order first (the first hint is the likeliest), then name.
        let mut matching: Vec<(usize, String)> = names
            .into_iter()
            .filter_map(|n| {
                let f = flat(&n);
                let pos = hints.iter().position(|h| {
                    if server {
                        f.strip_prefix(h.as_str()).is_some_and(|rest| SERVER_SUFFIXES.contains(&rest))
                    } else {
                        f == *h
                    }
                })?;
                Some((pos, n))
            })
            .collect();
        matching.sort_by_key(|(p, _)| *p);
        for (_, name) in matching {
            let path = dir.join(&name);
            match check(&path, game) {
                Ok(()) => {
                    let build_id = std::iter::once(game.app_id)
                        .chain(game.server_apps.iter().copied())
                        .find_map(|a| install_build_id(&path, a));
                    return Resolved {
                        install: Some(Install {
                            path,
                            found: Found::Convention { games_dir: dir.clone(), name, server },
                            build_id,
                        }),
                        notes,
                    };
                }
                Err(e) => notes.push(format!("games folder: {e}")),
            }
        }
    }
    Resolved { install: None, notes }
}

/// The whole discovery below config and settings, client before
/// dedicated server: Steam's libraries, the games folder's client
/// folders, then SteamCMD's default places (`SteamGame::server_apps`) and
/// the games folder's server folders.
pub fn find_in_places(places: &Places, game: &SteamGame) -> Resolved {
    let mut notes = Vec::new();
    let take = |mut r: Resolved, notes: &mut Vec<String>| -> Option<Resolved> {
        notes.append(&mut r.notes);
        r.install.is_some().then_some(Resolved { install: r.install, notes: Vec::new() })
    };
    let found = |mut r: Resolved, notes: Vec<String>| {
        r.notes = notes;
        r
    };
    if let Some(r) = take(find_in_steam(&places.steam, game), &mut notes) {
        return found(r, notes);
    }
    if let Some(r) = take(find_by_convention(&places.games, game, false), &mut notes) {
        return found(r, notes);
    }
    for &app in game.server_apps {
        let server = SteamGame { app_id: app, ..*game };
        let mut r = find_in_steam(&places.steamcmd, &server);
        // Steam's "no folder" and "no library has it" notes would only
        // say what the client's search said; keep the ones about a
        // manifest that was found.
        r.notes.retain(|n| !n.starts_with("no Steam"));
        if let Some(i) = &mut r.install
            && let Found::Steam { root, library, library_path } = i.found.clone()
        {
            i.found = Found::SteamCmd { app_id: app, root, library, library_path };
        }
        if let Some(r) = take(r, &mut notes) {
            return found(r, notes);
        }
    }
    if let Some(r) = take(find_by_convention(&places.games, game, true), &mut notes) {
        return found(r, notes);
    }
    if notes.is_empty() {
        notes.push(if places.steam.is_empty() && places.games.is_empty() {
            "no Steam folder or games folder found".into()
        } else {
            format!("no Steam library, SteamCMD place or games folder has {} (app {})", game.name, game.app_id)
        });
    }
    Resolved { install: None, notes }
}

/// Steam folders to search: `$CHIMERA_STEAM_ROOT` if set, else where
/// Steam lives on this system; existing ones, each once.
pub fn steam_roots() -> Vec<PathBuf> {
    let candidates = match crate::env::var_os(STEAM_ROOT_ENV) {
        Some(v) => std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()).collect(),
        None => platform_steam_roots(),
    };
    existing(candidates)
}

/// Where Steam installs itself on this system (existing or not).
#[allow(clippy::vec_init_then_push)]
fn platform_steam_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        out.extend(windows_registry_steam_path());
        if let Some(pf) = std::env::var_os("ProgramFiles(x86)") {
            out.push(PathBuf::from(pf).join("Steam"));
        }
        out.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("Library/Application Support/Steam"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".steam/steam"));
        out.push(home.join(".steam/root"));
        out.push(home.join(".local/share/Steam"));
        // Flatpak.
        out.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
        out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    out
}

/// `HKCU\Software\Valve\Steam` `SteamPath`.
#[cfg(windows)]
fn windows_registry_steam_path() -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (key, value) = (wide(r"Software\Valve\Steam"), wide("SteamPath"));
    let mut len: u32 = 0;
    // SAFETY: the key and value names are NUL-terminated UTF-16; a null
    // buffer asks only for the size, written to `len`.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut len,
        )
    };
    if err != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2)];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `len` bytes.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if err != 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..n]);
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The Steam build of the install at `path` of app `app_id`: its own
/// `steamapps/` (an install made with SteamCMD's `force_install_dir`),
/// else its Steam library's (`build_id_near`).
pub fn install_build_id(path: &Path, app_id: u32) -> Option<String> {
    read_manifest(&path.join("steamapps").join(format!("appmanifest_{app_id}.acf")))
        .and_then(|m| m.build_id)
        .or_else(|| build_id_near(path, &SteamGame { app_id, name: "", required: &[], server_apps: &[], folders: &[] }))
}

/// The Steam build of an install found some other way: its library's
/// manifest, when it sits in `<library>/steamapps/common/<dir>`.
pub fn build_id_near(path: &Path, game: &SteamGame) -> Option<String> {
    let path = fs::canonicalize(path).ok()?;
    let steamapps = path.parent()?.parent()?;
    read_manifest(&steamapps.join(format!("appmanifest_{}.acf", game.app_id)))?.build_id
}

// ---------------------------------------------------------------------------
// The user's settings.

/// The user's own Chimera files (outside any checkout): Linux
/// `~/.local/share/chimera`, Windows `%APPDATA%\chimera` (the old `mashup`
/// folder while only it exists, `env::user_folder`; the cfg folder,
/// `console::cfg_dir`, is its `cfg/`).
pub fn user_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(crate::env::user_folder(&base))
}

/// The settings file: `$CHIMERA_SETTINGS`, else `settings.toml` in
/// `user_dir`.
pub fn settings_path() -> Option<PathBuf> {
    crate::env::var_os(SETTINGS_ENV).map(PathBuf::from).or_else(|| Some(user_dir()?.join("settings.toml")))
}

/// What the settings file holds: install paths by game, as
/// `chimera.local.toml` names them (never keys).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default)]
    pub games: BTreeMap<String, SavedGame>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedGame {
    pub path: PathBuf,
}

impl Settings {
    /// The file's settings; none if it's missing or unreadable.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = format!("# Chimera's own settings (written by the game; see chimera.local.example.toml).\n{text}");
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Remember (or with None forget) `game`'s install in the settings file.
pub fn save_install(settings: &Path, game: &str, path: Option<&Path>) -> Result<(), String> {
    let mut s = Settings::load(settings);
    match path {
        Some(p) => {
            s.games.insert(game.to_string(), SavedGame { path: p.to_path_buf() });
        }
        None => {
            s.games.remove(game);
        }
    }
    s.save(settings)
}

// ---------------------------------------------------------------------------
// The whole search.

/// Find `game`'s install: config, then the settings file, then Steam
/// (`steam`: the roots to search, or None to skip Steam); the SteamCMD
/// places and games folders are `resolve_in`'s.
pub fn resolve_with(
    config: &LocalConfig,
    settings: Option<&Path>,
    steam: Option<&[PathBuf]>,
    game: &str,
    sg: Option<&SteamGame>,
) -> Resolved {
    let places = steam.map(|s| Places { steam: s.to_vec(), ..Places::default() });
    resolve_in(config, settings, places.as_ref(), game, sg)
}

/// `resolve_with` with all the places (`places`: None skips them).
pub fn resolve_in(
    config: &LocalConfig,
    settings: Option<&Path>,
    places: Option<&Places>,
    game: &str,
    sg: Option<&SteamGame>,
) -> Resolved {
    let with_build =
        |path: PathBuf, found: Found| Install { build_id: sg.and_then(|g| build_id_near(&path, g)), path, found };
    let mut notes = Vec::new();
    if let Some(g) = config.games.get(game)
        && let Some(path) = &g.path
    {
        let path = expand_home(path);
        if let Some(sg) = sg
            && let Err(e) = check(&path, sg)
        {
            notes.push(format!("config path doesn't check out: {e}"));
        }
        return Resolved { install: Some(with_build(path, Found::Config(config.file.clone()))), notes };
    }
    if let Some(file) = settings
        && let Some(saved) = Settings::load(file).games.get(game)
    {
        let path = expand_home(&saved.path);
        match sg.map_or(Ok(()), |sg| check(&path, sg)) {
            Ok(()) => {
                return Resolved { install: Some(with_build(path, Found::Saved(file.to_path_buf()))), notes };
            }
            Err(e) => notes.push(format!("saved path doesn't check out: {e}")),
        }
    }
    let (Some(sg), Some(places)) = (sg, places) else {
        notes.push(format!("{NO_PATH} for `{game}` in chimera.local.toml"));
        return Resolved { install: None, notes };
    };
    let mut found = find_in_places(places, sg);
    notes.append(&mut found.notes);
    found.notes = notes;
    found
}

const NO_PATH: &str = "no install path";

/// Steam searches made this run, by game (the folders don't move while
/// the game runs; `forget_steam` searches again).
static STEAM_FOUND: Mutex<BTreeMap<String, Resolved>> = Mutex::new(BTreeMap::new());

/// `resolve_with` from the environment: `$CHIMERA_<GAME>_ROOT` first
/// (`root_env`; taken as it is, like the config's path, with a note when
/// it doesn't check out), then the settings file (`settings_path`), the
/// Steam roots, SteamCMD places and games folders (`Places::system`,
/// searched once per run).
pub fn resolve(config: &LocalConfig, game: &str, sg: Option<&SteamGame>) -> Resolved {
    let var = root_env(game);
    if let Some(path) = crate::env::var_os(&var).filter(|p| !p.is_empty()) {
        let path = expand_home(Path::new(&path));
        let mut notes = Vec::new();
        if let Some(sg) = sg
            && let Err(e) = check(&path, sg)
        {
            notes.push(format!("${var} doesn't check out: {e}"));
        }
        let build_id = sg.and_then(|g| install_build_id(&path, g.app_id));
        return Resolved { install: Some(Install { path, found: Found::Env(var), build_id }), notes };
    }
    let settings = settings_path();
    let mut r = resolve_with(config, settings.as_deref(), None, game, sg);
    let Some(sg) = sg.filter(|_| r.install.is_none()) else {
        return r;
    };
    let steam = STEAM_FOUND
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(game.to_string())
        .or_insert_with(|| find_in_places(&Places::system(), sg))
        .clone();
    r.notes.retain(|n| !n.starts_with(NO_PATH));
    r.notes.extend(steam.notes);
    r.install = steam.install;
    r
}

/// Search Steam again on the next `resolve` (after the user installs or
/// moves the game).
pub fn forget_steam() {
    STEAM_FOUND.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    static CSS: SteamGame = SteamGame {
        app_id: 240,
        name: "Counter-Strike: Source",
        required: &["cstrike/cstrike_pak_dir.vpk", "hl2/hl2_misc_dir.vpk"],
        server_apps: &[232330],
        folders: &["cs_source", "css", "counter-strike-source"],
    };

    /// A scratch folder, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("chimera-install-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn css() -> &'static SteamGame {
        &CSS
    }

    /// A CS:S install in `library` (its files as empty stand-ins) with
    /// its manifest.
    fn install_css(library: &Path, flags: u32) -> PathBuf {
        let dir = library.join("steamapps/common/Counter-Strike Source");
        for f in css().required {
            write(&dir.join(f), "");
        }
        write(
            &library.join("steamapps/appmanifest_240.acf"),
            &format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"240\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"Counter-Strike Source\"\n\t\"buildid\"\t\t\"12345\"\n}}\n"
            ),
        );
        dir
    }

    fn vdf_path(p: &Path) -> String {
        p.display().to_string().replace('\\', "\\\\")
    }

    #[test]
    fn finds_the_app_in_the_second_library() {
        let s = Scratch::new("two-libs");
        let root = s.0.join("Steam");
        let second = s.0.join("Games/SteamLibrary");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let dir = install_css(&second, 4);
        write(
            &root.join("steamapps/libraryfolders.vdf"),
            &format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"228980\"\t\t\"1\"\n\t\t}}\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"240\"\t\t\"4775539566\"\n\t\t}}\n\t}}\n}}\n",
                vdf_path(&root),
                vdf_path(&second)
            ),
        );
        let r = find_in_steam(&[root.clone()], css());
        let i = r.install.clone().expect("found");
        assert_eq!(i.path, dir);
        assert_eq!(i.build_id.as_deref(), Some("12345"));
        assert!(matches!(i.found, Found::Steam { library: 1, .. }), "{:?}", i.found);
        assert!(r.notes.is_empty(), "{:?}", r.notes);
        assert!(r.describe("CS:S").contains("Steam library 1"), "{}", r.describe("CS:S"));
    }

    #[test]
    fn old_format_library_folders() {
        let s = Scratch::new("old-format");
        let root = s.0.join("Steam");
        let second = s.0.join("lib2");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let dir = install_css(&second, 4);
        write(
            &root.join("steamapps/libraryfolders.vdf"),
            &format!(
                "\"LibraryFolders\"\n{{\n\t\"TimeNextStatsReport\"\t\t\"1600000000\"\n\t\"ContentStatsID\"\t\t\"-123\"\n\t\"1\"\t\t\"{}\"\n}}\n",
                vdf_path(&second)
            ),
        );
        let libs = libraries(&root);
        assert_eq!(libs.len(), 2);
        assert_eq!((libs[0].index, &libs[0].path), (0, &root));
        assert_eq!((libs[1].index, &libs[1].path), (1, &second));
        let i = find_in_steam(&[root], css()).install.expect("found");
        assert_eq!(i.path, dir);
        assert!(matches!(i.found, Found::Steam { library: 1, .. }));
    }

    #[test]
    fn windows_paths_with_escaped_backslashes() {
        let s = Scratch::new("windows-paths");
        write(
            &s.0.join("steamapps/libraryfolders.vdf"),
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"240\"\t\t\"1\"\n\t\t}\n\t}\n}\n",
        );
        let libs = libraries(&s.0);
        let paths: Vec<String> = libs.iter().map(|l| l.path.display().to_string()).collect();
        assert!(paths.contains(&r"C:\Program Files (x86)\Steam".to_string()), "{paths:?}");
        assert!(paths.contains(&r"D:\SteamLibrary".to_string()), "{paths:?}");
        assert_eq!(libs.iter().find(|l| l.index == 1).unwrap().apps, vec![240]);
    }

    #[test]
    fn missing_manifest_is_not_found_and_said() {
        let s = Scratch::new("no-manifest");
        write(
            &s.0.join("steamapps/libraryfolders.vdf"),
            &format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" \"apps\" {{ \"240\" \"1\" }} }} }}",
                vdf_path(&s.0)
            ),
        );
        let r = find_in_steam(&[s.0.clone()], css());
        assert!(r.install.is_none());
        assert!(r.notes.iter().any(|n| n.contains("no appmanifest_240.acf")), "{:?}", r.notes);
    }

    #[test]
    fn partially_installed_is_skipped() {
        let s = Scratch::new("partial");
        install_css(&s.0, 1026);
        let r = find_in_steam(&[s.0.clone()], css());
        assert!(r.install.is_none());
        assert!(r.notes.iter().any(|n| n.contains("isn't fully installed")), "{:?}", r.notes);
        // An update in progress on an installed game keeps the bit.
        install_css(&s.0, 4 | 2);
        assert!(find_in_steam(&[s.0.clone()], css()).install.is_some());
    }

    #[test]
    fn a_folder_without_the_games_files_is_rejected() {
        let s = Scratch::new("no-files");
        install_css(&s.0, 4);
        fs::remove_file(s.0.join("steamapps/common/Counter-Strike Source/hl2/hl2_misc_dir.vpk")).unwrap();
        let r = find_in_steam(&[s.0.clone()], css());
        assert!(r.install.is_none());
        assert!(r.notes.iter().any(|n| n.contains("hl2/hl2_misc_dir.vpk")), "{:?}", r.notes);
        assert!(find_in_steam(&[], css()).notes.iter().any(|n| n.contains("no Steam folder")));
    }

    #[test]
    fn config_then_saved_then_steam() {
        let s = Scratch::new("precedence");
        let steam = s.0.join("steam");
        let steam_dir = install_css(&steam, 4);
        let saved_dir = install_css(&s.0.join("saved"), 4);
        let settings = s.0.join("user/settings.toml");
        let roots = [steam.clone()];

        // Nothing configured or saved: Steam.
        let none = LocalConfig::default();
        let r = resolve_with(&none, Some(&settings), Some(&roots), "cs_source", Some(css()));
        assert_eq!(r.install.as_ref().unwrap().path, steam_dir);

        // Saved beats Steam.
        save_install(&settings, "cs_source", Some(&saved_dir)).unwrap();
        let r = resolve_with(&none, Some(&settings), Some(&roots), "cs_source", Some(css()));
        let i = r.install.unwrap();
        assert_eq!(i.path, saved_dir);
        assert_eq!(i.found, Found::Saved(settings.clone()));
        assert_eq!(i.build_id.as_deref(), Some("12345"), "the saved install's manifest");

        // The config beats both, even a path that doesn't check out.
        let config: LocalConfig = toml::from_str("[games.cs_source]\npath = \"/nowhere/css\"").unwrap();
        let r = resolve_with(&config, Some(&settings), Some(&roots), "cs_source", Some(css()));
        assert_eq!(r.install.unwrap().path, PathBuf::from("/nowhere/css"));
        assert!(r.notes.iter().any(|n| n.contains("config path")));

        // A saved path that no longer checks out falls through to Steam.
        save_install(&settings, "cs_source", Some(&s.0.join("gone"))).unwrap();
        let r = resolve_with(&none, Some(&settings), Some(&roots), "cs_source", Some(css()));
        assert_eq!(r.install.unwrap().path, steam_dir);
        assert!(r.notes.iter().any(|n| n.contains("saved path")));

        // Forgetting keeps other games' entries.
        save_install(&settings, "other", Some(Path::new("/x"))).unwrap();
        save_install(&settings, "cs_source", None).unwrap();
        let kept = Settings::load(&settings);
        assert!(kept.games.contains_key("other") && !kept.games.contains_key("cs_source"));

        // A game Steam doesn't have here: config only.
        let r = resolve_with(&none, Some(&settings), Some(&roots), "cs_source_server", None);
        assert!(r.install.is_none());
    }

    /// The game's files, empty, in `dir`.
    fn files_in(dir: &Path) -> PathBuf {
        for f in css().required {
            write(&dir.join(f), "");
        }
        dir.to_path_buf()
    }

    fn places(steam: &[&Path], steamcmd: &[&Path], games: &[&Path]) -> Places {
        let v = |s: &[&Path]| s.iter().map(|p| p.to_path_buf()).collect();
        Places { steam: v(steam), steamcmd: v(steamcmd), games: v(games) }
    }

    #[test]
    fn games_folder_finds_the_client_by_any_of_its_names() {
        let s = Scratch::new("conv-client");
        let games = s.0.join("games");
        let dir = files_in(&games.join("Counter-Strike_Source"));
        // Other folders that merely look alike stay out.
        files_in(&games.join("css_assembled"));
        let r = find_in_places(&places(&[], &[], &[&games]), css());
        let i = r.install.expect("found");
        assert_eq!(i.path, dir);
        assert_eq!(i.found, Found::Convention { games_dir: games.clone(), name: "Counter-Strike_Source".into(), server: false });
        assert!(i.found.to_string().contains("games folder convention, client"), "{}", i.found);
    }

    #[test]
    fn games_folder_prefers_the_client_to_a_dedicated_server() {
        let s = Scratch::new("conv-order");
        let games = s.0.join("games");
        let ds = files_in(&games.join("css-ds"));
        let r = find_in_places(&places(&[], &[], &[&games]), css());
        let i = r.install.expect("the server alone is found");
        assert_eq!((i.path.clone(), &i.found), (ds, &Found::Convention { games_dir: games.clone(), name: "css-ds".into(), server: true }));
        // The client appears: it wins, whatever the folders' order.
        let client = files_in(&games.join("css"));
        files_in(&games.join("aaa")); // not a name of the game
        let i = find_in_places(&places(&[], &[], &[&games]), css()).install.unwrap();
        assert_eq!(i.path, client);
        // Every server spelling is one.
        for name in ["css_ds", "CSS-server", "css_dedicated", "counter-strike-source_ds"] {
            let s2 = Scratch::new("conv-spell");
            let g = s2.0.join("g");
            files_in(&g.join(name));
            let i = find_in_places(&places(&[], &[], &[&g]), css()).install.unwrap_or_else(|| panic!("{name}"));
            assert!(matches!(i.found, Found::Convention { server: true, .. }), "{name}: {:?}", i.found);
        }
    }

    #[test]
    fn steam_client_then_games_client_then_steamcmd_then_games_server() {
        let s = Scratch::new("order");
        let steam = s.0.join("steam");
        let steamcmd = s.0.join("steamcmd");
        let games = s.0.join("games");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        let server_manifest = |lib: &Path, app: u32, dir: &str| {
            write(
                &lib.join(format!("steamapps/appmanifest_{app}.acf")),
                &format!("\"AppState\"\n{{\n\t\"appid\"\t\"{app}\"\n\t\"StateFlags\"\t\"4\"\n\t\"installdir\"\t\"{dir}\"\n\t\"buildid\"\t\"777\"\n}}\n"),
            );
            files_in(&lib.join("steamapps/common").join(dir))
        };
        let ds_games = files_in(&games.join("css_ds"));
        let all = places(&[&steam], &[&steamcmd], &[&games]);

        // Only the games folder's server: it is the last resort.
        let i = find_in_places(&all, css()).install.unwrap();
        assert_eq!(i.path, ds_games);

        // SteamCMD's default place beats it.
        let ds_cmd = server_manifest(&steamcmd, 232330, "Counter-Strike Source Dedicated Server");
        let i = find_in_places(&all, css()).install.unwrap();
        assert_eq!(i.path, ds_cmd);
        assert!(matches!(i.found, Found::SteamCmd { app_id: 232330, library: 0, .. }), "{:?}", i.found);
        assert_eq!(i.build_id.as_deref(), Some("777"), "the server app's manifest gives the build");
        assert!(i.found.to_string().contains("SteamCMD"), "{}", i.found);

        // A client in the games folder beats both servers.
        let client = files_in(&games.join("counter-strike-source"));
        assert_eq!(find_in_places(&all, css()).install.unwrap().path, client);

        // A client in Steam's libraries beats it.
        let in_steam = install_css(&steam, 4);
        let i = find_in_places(&all, css()).install.unwrap();
        assert_eq!(i.path, in_steam);
        assert!(matches!(i.found, Found::Steam { .. }), "{:?}", i.found);
    }

    #[test]
    fn a_games_folder_that_does_not_check_out_is_said_and_skipped() {
        let s = Scratch::new("conv-bad");
        let games = s.0.join("games");
        fs::create_dir_all(games.join("css")).unwrap();
        let ds = files_in(&games.join("css_server"));
        let r = find_in_places(&places(&[], &[], &[&games]), css());
        assert_eq!(r.install.as_ref().unwrap().path, ds);
        assert!(r.notes.iter().any(|n| n.contains("games folder") && n.contains("css")), "{:?}", r.notes);
        let none = find_in_places(&places(&[], &[], &[]), css());
        assert!(none.install.is_none() && !none.notes.is_empty(), "{:?}", none.notes);
    }

    #[test]
    fn config_and_saved_beat_the_conventions() {
        let s = Scratch::new("conv-precedence");
        let games = s.0.join("games");
        let conv = files_in(&games.join("css"));
        let saved = install_css(&s.0.join("saved"), 4);
        let settings = s.0.join("user/settings.toml");
        let p = places(&[], &[], &[&games]);
        let none = LocalConfig::default();
        assert_eq!(resolve_in(&none, Some(&settings), Some(&p), "cs_source", Some(css())).install.unwrap().path, conv);
        save_install(&settings, "cs_source", Some(&saved)).unwrap();
        assert_eq!(resolve_in(&none, Some(&settings), Some(&p), "cs_source", Some(css())).install.unwrap().path, saved);
        let config: LocalConfig = toml::from_str("[games.cs_source]\npath = \"/nowhere/css\"").unwrap();
        let r = resolve_in(&config, Some(&settings), Some(&p), "cs_source", Some(css()));
        assert_eq!(r.install.unwrap().path, PathBuf::from("/nowhere/css"));
        // No places: no conventions.
        assert!(resolve_in(&none, None, None, "cs_source", Some(css())).install.is_none());
    }

    #[test]
    fn this_systems_places_are_listed_and_exist() {
        let p = Places::system();
        for d in p.steam.iter().chain(&p.steamcmd).chain(&p.games) {
            assert!(d.is_dir(), "{}", d.display());
        }
    }
}
