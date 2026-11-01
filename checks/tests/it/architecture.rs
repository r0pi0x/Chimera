//! Structural tests: module layering and repository rules from
//! docs/ARCHITECTURE.md and CLAUDE.md. Failure messages say how to fix the
//! problem, because agents read them.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Top-level module -> top-level modules it may use via `crate::`.
/// Keep in sync with the layer diagram in docs/ARCHITECTURE.md.
const ALLOWED: &[(&str, &[&str])] = &[
    ("core", &[]),
    // Environment variables and per-user folders under their new and old
    // names (`CHIMERA_*`/`MASHUP_*`): std only.
    ("env", &[]),
    // Performance counters and distributions: Bevy only.
    ("metrics", &[]),
    // `world.query` with its state kept between calls: Bevy only.
    ("query_cache", &[]),
    // Values worked out once and kept by their owner (`Memos`): std only.
    ("memo", &[]),
    ("console", &["core", "env", "query_cache", "console_names"]),
    ("console_names", &[]),
    // The console's scope (the game played: the local player's identity,
    // else the default game) and the games' console namespaces.
    ("console_scope", &["core", "console", "game_registry", "slots"]),
    // Game events and the server log: what the rules and the network tell
    // plugins and admins happened (players named by the gameplay's game's
    // `LogSubjects`).
    ("game_log", &["core", "console", "gameplay"]),
    ("mount", &["env"]),
    // A player profile file (sections, `Key = value`): std only.
    ("profile", &[]),
    // What a game tells the shell about itself (its install, map cycle,
    // tick, binds, movement cvars): data the game registers.
    ("game_registry", &["mount"]),
    // Which game fills each match-wide slot (rules, HUD, binds, tick) and
    // the sets that gate a game's systems by it.
    ("gameplay", &["core", "game_registry"]),
    // What each game lends a game made of several (plain data), its
    // announcer pack and minigame cue pack among them.
    ("game_parts", &["announcer", "cues"]),
    // Announcers' packs, the server's choice and kill facts (plain data).
    ("announcer", &[]),
    // Minigame cue packs: sounds by cue key (plain data).
    ("cues", &[]),
    ("slots", &["core"]),
    ("character", &["core", "slots"]),
    ("movement", &["core", "slots", "character"]),
    ("greybox", &["core"]),
    ("map", &["core"]),
    // A background-music player (two stream slots, fades): pure state, Bevy
    // only; a game with streamed music uses it.
    ("music", &[]),
    // Audio: plays what the simulation's sounds say (`map::sound`); nothing
    // in the simulation may use it.
    ("audio", &["core", "map"]),
    // Rendering: draws what the map spawns and simulates (the drawn phases
    // of spawning a map, through `map::MapDrawer`, what the simulation
    // makes in play from its drawn-part requests, effects, culling, water,
    // shadows, sky, view models, the materials and shaders); nothing in the
    // simulation may use it, and the simulation names no drawing type
    // (`simulation_builds_no_drawn_parts`).
    ("render", &["core", "map"]),
    // Builds `map::MapPlugin` from the map and its presentation (`audio`,
    // `render`), its systems in `core`'s sets.
    ("map_plugin", &["core", "map", "audio", "render"]),
    // The installs as asset sources (`<game>://<path>` through a game's
    // mount) and what texture loaders ask of the device; used by apps and
    // games' front ends.
    ("assets", &["core", "mount", "game_registry", "render"]),
    ("weapon", &["core", "console", "map"]),
    // The rule-module interface (chimera/sim): a game registers its modes
    // by name; `rules::module` at its old path.
    ("rule_module", &["console"]),
    // The mode runtime (chimera/sim): modes as data and hooks on one match
    // whose phases it steps; which game plays from the gameplay.
    ("mode", &["core", "console", "gameplay"]),
    // Minigames (chimera/sim): definitions as data played on the mode
    // runtime, their spawning and weapons, the gameplay switched to them.
    (
        "minigame",
        &["core", "console", "map", "mode", "spawning", "weapon", "slots", "gameplay", "game_registry", "identity"],
    ),
    // A character's game identity following its weapon (chimera/sim): its
    // movement switched in the predicted tick, its identity, the local
    // player's HUD slot.
    ("identity", &["core", "map", "movement", "slots", "weapon", "gameplay", "game_registry"]),
    // Every game's respawn (chimera/sim): the dead held, a character
    // revived at a place (out of a toggled noclip).
    ("spawning", &["core", "movement"]),
    // The announcer's counting of kills (chimera/sim): from deaths and the
    // guns' hit regions, the server's `sv_announcer`.
    ("announce", &["core", "console", "map", "weapon"]),
    // Server data documents (chimera/sim): a game's derived data as named
    // documents a network client takes from the server (weapon sets
    // rebuilt in place).
    ("server_docs", &["weapon"]),
    // The test kit (chimera/sim, feature `testkit`): a weapon set as data
    // and the base on the greybox, for tests; nothing outside the crate's
    // frameworks.
    ("testkit", &["lib", "movement", "weapon"]),
    // Round objectives (bomb, hostages): zones from the map, NPCs as
    // characters, the bomb as a weapon.
    (
        "objectives",
        &["core", "console", "slots", "character", "map", "weapon", "gameplay"],
    ),
    ("rules", &["core", "console", "game_log", "gameplay", "weapon", "objectives"]),
    // Map logic (entity I/O, triggers, movers): game-independent; games
    // feed it through `map::MapData::entities`. `net`: it registers its
    // movers and HUD messages as its part of the network (`logic::net`).
    ("logic", &["core", "console", "map", "net"]),
    (
        "bot",
        &["core", "console", "slots", "character", "map", "weapon", "objectives"],
    ),
    // Network play: transport, replication, the join handshake. Above the
    // engine's simulation it serves, below Source's logic, the games, the
    // client and harness (docs/plans/active/multiplayer.md, section 3): a
    // game registers what it replicates and fills its hooks (`net::parts`).
    (
        "net",
        &[
            "core",
            "console",
            "game_log",
            "slots",
            "character",
            "movement",
            "map",
            "weapon",
        ],
    ),
    // The minigame runtime's part of the network (chimera/net): the match
    // and the players' ladder state replicated, a client starting the
    // server's minigame, the manifest's minigame and `change_match` (a
    // console command, as `net`'s). Above `net`, `minigame` and `mode`,
    // which `net` itself may not use; an app that plays minigames over the
    // network adds its plugin.
    ("minigame_net", &["core", "console", "net", "minigame", "mode"]),
    // The headless harness (chimera_world's `Sim`, chimera_net's `NetSim`,
    // cs_source's base and test maps). `lib` = crate-root items such as
    // `SimPlugins`.
    (
        "harness",
        &[
            "lib",
            "core",
            "gameplay",
            "console",
            "slots",
            "character",
            "movement",
            "greybox",
            "map",
            "weapon",
            "objectives",
            "rules",
            "bot",
            "logic",
            "net",
            "metrics",
        ],
    ),
    // The client shell: window, input, camera, console UI, the VGUI-like
    // widget layer, debug tools. Never a game's module, the app's
    // (`lib`, `games`, `greybox`: `client::AppHooks`) or the map logic
    // (`map::logic_view`): what it needs of a game is data the game
    // registers (`game_registry`, `core`'s vocabulary and hooks,
    // `debug_ui::GameDebug`). A game's menus and HUD are that game's
    // (`games_ui`), reached through `client::front_end`.
    (
        "client",
        &[
            "core",
            "console",
            "slots",
            "character",
            "movement",
            "map",
            "audio",
            "render",
            "weapon",
            "mount",
            "game_registry",
            "gameplay",
            "net",
            "metrics",
        ],
    ),
    // A game's front end (its crate's `ui`, behind its feature: its menus, panels and
    // HUD): built on the client shell; its own game's modules too. Nothing
    // else in the game uses it (`game_parts_do_not_use_their_ui`).
    (
        "games_ui",
        &[
            "lib",
            "core",
            "console",
            "slots",
            "character",
            "movement",
            "greybox",
            "map",
            "audio",
            "render",
            "assets",
            "weapon",
            "objectives",
            "rules",
            "bot",
            "mount",
            "profile",
            "game_registry",
            "gameplay",
            "games",
            "logic",
            "net",
            "metrics",
            "client",
        ],
    ),
    // Game plugins: may use the shared layers, never another game. `rules`
    // and `rule_module`: a game registers its rule modules (CS:S through
    // `rules::module`, Combat Arms through `rule_module`); `net` and `bot`:
    // its part of the network (`game::net`: its round, bomb and hostages
    // replicated, its join flow and bots behind the network's hooks).
    (
        "games",
        &[
            "core",
            "console",
            "slots",
            "character",
            "movement",
            "mount",
            "profile",
            "game_registry",
            "gameplay",
            "map",
            "music",
            "weapon",
            "objectives",
            "rules",
            "rule_module",
            "mode",
            "spawning",
            "bot",
            "net",
        ],
    ),
    // Binaries use the library through `mashup::`, not `crate::`.
    ("bin", &[]),
];

/// The crates' source folders cut out of `mashup`, scanned with `src/`
/// (their modules keep their names in `ALLOWED`; a game crate's `game`
/// module is `games`, as `src/games/<game>/` was). The CS:S app's glue
/// (`APP_SRC`) keeps the names it had as `mashup`'s.
const CRATE_SRC: &[&str] = &[
    APP_SRC,
    "chimera/core/src",
    "chimera/world/src",
    "chimera/present/src",
    "chimera/sim/src",
    "chimera/client/src",
    "chimera/net/src",
    "source/logic/src",
    "games/cs_source/src",
    "games/combat_arms/src",
];

/// The CS:S app's glue (crate `cs_source_app`): `lib.rs` (`SimPlugins`,
/// `client_hooks`, the harness's and greybox's old paths) and `client.rs`
/// are the crate root's (`lib`), `main.rs` the game binary's; its other
/// modules keep the names they had in `mashup` (`games`, `net`, ...).
/// (`harness` and `greybox` are chimera_world's modules now, with
/// chimera_net's and cs_source's `harness`.)
const APP_SRC: &str = "apps/cs_source/src";

/// The repository's root (this crate is checks/).
pub(crate) fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub(crate) fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The top-level module a file belongs to: `src/greybox.rs` -> `greybox`.
/// A game's front end (`src/games/<game>/ui/...`, a game crate's
/// `games/<game>/src/ui/...`) is `games_ui`; a game crate's parts
/// (`games/<game>/src/game/...`) are `games`.
fn module_of(src: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(src).ok()?;
    let mut parts = rel.components().map(|c| c.as_os_str().to_str().unwrap_or(""));
    let first = parts.next()?;
    // The app's `client` re-exports the shell (chimera_client) with the
    // app's own `ClientPlugin` and CS:S's HUD: app glue, as the crate
    // root.
    if rel == Path::new("client.rs") && src == root().join(APP_SRC) {
        return Some("lib".to_string());
    }
    if first == "games" && parts.nth(1) == Some("ui") {
        return Some("games_ui".to_string());
    }
    if first == "game" && crate_game(src).is_some() {
        return Some("games".to_string());
    }
    if (first == "ui" || first == "ui.rs") && crate_game(src).is_some() {
        return Some("games_ui".to_string());
    }
    Some(first.trim_end_matches(".rs").to_string())
}

/// The game a game crate's source folder (`games/<game>/src`) holds.
fn crate_game(src: &Path) -> Option<String> {
    let game = src.parent()?;
    (game.parent()?.file_name()? == "games" && src.file_name()? == "src")
        .then(|| game.file_name().unwrap().to_string_lossy().to_string())
}

/// The games: `src/games/<name>/` folders and game crates
/// (`games/<name>/`).
fn game_names(src: &Path) -> BTreeSet<String> {
    [src.join("games"), root().join("games")]
        .iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect()
}

fn ident(s: &str) -> String {
    s.trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// The client shell (`src/client`) names no game: it learns what it needs
/// of one from the game registry (`game_registry::Games`: the default
/// game's install, maps, map cycle, tick, binds, movement cvars) and
/// reaches its front end through `client::front_end`.
fn client_names_game(rel: &str, game: &str) -> String {
    format!(
        "{rel}: the client shell uses game `{game}`. The shell knows a game only as data it registers \
         (`game_registry::GameInfo`, read through `game_registry::Games`; add a field there) and reaches \
         its menus, panels and HUD (its crate's `ui`, games/{game}/src/ui/) through `client::front_end` hooks; the app adds \
         the game's plugins (main.rs)."
    )
}

/// Top-level paths used after `crate::` in `code` (comments already
/// stripped). `crate::{a::X, b}` yields `a` and `b`; `crate::games::cs::X`
/// yields `games::cs`. Crate-root items (`crate::SimPlugins`) yield `lib`.
fn crate_refs(code: &str, modules: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut add = |path: &str| {
        let first = ident(path);
        if first.is_empty() {
            return;
        }
        if first == "games" {
            let rest = path.trim_start()["games".len()..].trim_start_matches("::");
            out.insert(format!("games::{}", ident(rest)));
        } else if modules.contains(&first) {
            out.insert(first);
        } else {
            out.insert("lib".to_string());
        }
    };
    let mut rest = code;
    while let Some(i) = rest.find("crate::") {
        rest = &rest[i + "crate::".len()..];
        if let Some(group) = rest.strip_prefix('{') {
            // Split the group at top-level commas.
            let (mut depth, mut item_start) = (0usize, 0usize);
            for (j, c) in group.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => {
                        add(&group[item_start..j]);
                        break;
                    }
                    '}' => depth -= 1,
                    ',' if depth == 0 => {
                        add(&group[item_start..j]);
                        item_start = j + 1;
                    }
                    _ => {}
                }
            }
        } else {
            add(rest);
        }
    }
    out
}

#[test]
fn module_layering() {
    let src = root().join("src");
    // The crates cut out keep their modules' names (`chimera_core`'s
    // `crate::core`, `chimera_world`'s `crate::map`, `chimera_present`'s
    // `crate::render`, `source_logic`'s `crate::logic`, re-exported by
    // `mashup` at the same paths), so one table covers them all.
    let mut files = Vec::new();
    for dir in CRATE_SRC.iter().copied().chain(["src"]) {
        let mut more = Vec::new();
        rust_files(&root().join(dir), &mut more);
        files.extend(more.into_iter().map(|f| (root().join(dir), f)));
    }
    let modules: BTreeSet<String> = ALLOWED.iter().map(|(m, _)| m.to_string()).collect();
    let games = game_names(&src);
    let mut errors = Vec::new();

    for (base, file) in files {
        let Some(module) = module_of(&base, &file) else {
            continue;
        };
        if module == "lib" || module == "main" {
            continue;
        }
        let Some((_, allowed)) = ALLOWED.iter().find(|(m, _)| *m == module) else {
            errors.push(format!(
                "{}: module `{module}` has no layering rule. Add it to ALLOWED in checks/tests/it/architecture.rs \
                 and to the layer diagram in docs/ARCHITECTURE.md.",
                file.display()
            ));
            continue;
        };
        let text = fs::read_to_string(&file).unwrap();
        // Strip line comments and join, so grouped imports can span lines.
        let mut code = text
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        // A game crate's own parts (`crate::game::`) are its game's module.
        let crate_game = crate_game(&base);
        if let Some(game) = &crate_game {
            code = code.replace("crate::game::", &format!("crate::games::{game}::"));
        }
        let rel = file.strip_prefix(root()).unwrap().to_string_lossy().replace('\\', "/");
        // The shell names no game by any path (`::cs_source::`,
        // `mashup::games::cs_source::` as well as `crate::games::...`).
        if module == "client" {
            for game in &games {
                if code.contains(&format!("{game}::")) {
                    errors.push(client_names_game(&rel, game));
                }
            }
        }
        for r in crate_refs(&code, &modules) {
            // `games::load_map` and the like: the game-neutral map loading.
            let r = match r.strip_prefix("games::") {
                Some(name) if !games.contains(name) => "games".to_string(),
                _ => r,
            };
            if let Some(game) = r.strip_prefix("games::") {
                let own = file
                    .strip_prefix(src.join("games"))
                    .ok()
                    .and_then(|p| p.components().next());
                let own = own
                    .map(|c| c.as_os_str().to_string_lossy().trim_end_matches(".rs").to_string())
                    .or(crate_game.clone());
                if module == "client" {
                    errors.push(client_names_game(&rel, game));
                } else if own.as_deref() != Some(game) && module != "lib" {
                    errors.push(format!(
                        "{}: uses game `{game}`. Games must not depend on each other, and shared layers must \
                         not depend on any game. Move the shared concept into `core` (rule of three) or add a \
                         bridge; see docs/ARCHITECTURE.md.",
                        file.display()
                    ));
                }
                continue;
            }
            if r == module || allowed.contains(&r.as_str()) {
                continue;
            }
            errors.push(format!(
                "{}: `{module}` uses `crate::{r}`, which its layer may not depend on (allowed: {allowed:?}). \
                 Move the shared piece down a layer or invert the dependency (e.g. register through `slots`); \
                 see docs/ARCHITECTURE.md.",
                file.display()
            ));
        }
    }
    errors.dedup();
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// Drawing types: what builds or holds meshes and materials. The
/// simulation never names them; it says in plain data what its parts look
/// like and `render` draws them (docs/ARCHITECTURE.md, "Drawn-part
/// requests").
const DRAWING_TYPES: &[&str] = &[
    "Mesh3d",
    "MeshMaterial3d",
    "Assets<Mesh>",
    "StandardMaterial",
    "MaterialPlugin",
];

/// Files of the simulation's layers that still name one of
/// `DRAWING_TYPES` (file, why).
const MAP_DRAWING_EXCEPTIONS: &[(&str, &str)] = &[];

/// The simulation's layers (map, and a game's parts outside its `ui/`)
/// name no drawing type (`DRAWING_TYPES`) and hold no shader, so a
/// dedicated server can leave `render` out. (They can't use `render` at
/// all: `ALLOWED`.)
#[test]
fn simulation_builds_no_drawn_parts() {
    const SIM: &[&str] = &[
        "core",
        "map",
        "logic",
        "net",
        "weapon",
        "rules",
        "objectives",
        "bot",
        "movement",
        "rule_module",
        "mode",
        "spawning",
        "server_docs",
        "games",
    ];
    let mut files = Vec::new();
    for dir in CRATE_SRC.iter().copied().chain(["src"]) {
        let mut more = Vec::new();
        rust_files(&root().join(dir), &mut more);
        files.extend(more.into_iter().map(|f| (root().join(dir), f)));
    }
    let mut errors = Vec::new();
    let mut used_exceptions = BTreeSet::new();
    for (base, file) in files {
        let Some(module) = module_of(&base, &file) else {
            continue;
        };
        if !SIM.contains(&module.as_str()) {
            continue;
        }
        let rel = file.strip_prefix(root()).unwrap().to_string_lossy().replace('\\', "/");
        let code = fs::read_to_string(&file)
            .unwrap()
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        let named: Vec<&str> = DRAWING_TYPES.iter().copied().filter(|t| code.contains(t)).collect();
        if named.is_empty() {
            continue;
        }
        if MAP_DRAWING_EXCEPTIONS.iter().any(|(f, _)| *f == rel) {
            used_exceptions.insert(rel.clone());
            continue;
        }
        errors.push(format!(
            "{rel}: the simulation layer `{module}` names {named:?}, which build or hold drawn parts. \
             Say what the part looks like in plain data (a component or message `render` fulfils, \
             docs/ARCHITECTURE.md \"Drawn-part requests\") and draw it in `render`, or (with a TODO) \
             list the file in MAP_DRAWING_EXCEPTIONS in checks/tests/it/architecture.rs."
        ));
    }
    for (f, _) in MAP_DRAWING_EXCEPTIONS {
        if !used_exceptions.contains(*f) {
            errors.push(format!(
                "MAP_DRAWING_EXCEPTIONS lists {f}, which no longer names a drawing type: remove the entry."
            ));
        }
    }
    let wgsl: Vec<String> = fs::read_dir(root().join("chimera/world/src/map"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".wgsl") || n.ends_with(".wesl"))
        .collect();
    if !wgsl.is_empty() {
        errors.push(format!(
            "chimera/world/src/map holds shaders {wgsl:?}: shaders are `render`'s."
        ));
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// A game's parts (weapons, movement, map loading ...) work without its
/// front end: only its `ui/` uses `ui`.
#[test]
fn game_parts_do_not_use_their_ui() {
    let src = root().join("src");
    let mut errors = Vec::new();
    for game in game_names(&src) {
        let mut files = Vec::new();
        if src.join("games").join(&game).is_dir() {
            rust_files(&src.join("games").join(&game), &mut files);
        }
        let files = files.into_iter().map(|f| (src.clone(), f));
        // The game's crate (games/<game>/src), when it has one.
        let krate = root().join("games").join(&game).join("src");
        let mut crate_files = Vec::new();
        if krate.is_dir() {
            rust_files(&krate, &mut crate_files);
        }
        for (base, file) in files.chain(crate_files.into_iter().map(|f| (krate.clone(), f))) {
            if module_of(&base, &file).as_deref() == Some("games_ui") {
                continue;
            }
            let code: String = fs::read_to_string(&file)
                .unwrap()
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" ");
            let names_ui = [
                format!("{game}::ui::"),
                "super::ui::".into(),
                "self::ui::".into(),
                "crate::ui::".into(),
            ]
            .iter()
            .any(|p| code.contains(p.as_str()));
            if names_ui {
                errors.push(format!(
                    "{}: uses `{game}::ui`, the game's front end. A game's parts must work without its menus \
                     and HUD (a dedicated server, another game's front end): move what both need out of `ui/`.",
                    file.display()
                ));
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

fn tracked_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root())
        .output()
        .expect("git ls-files");
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Other games' assets must never be committed (README, "Assets, code and legal").
#[test]
fn no_game_assets_tracked() {
    const FORBIDDEN: &[&str] = &[
        "vpk", "bsp", "vtf", "vmt", "mdl", "vvd", "vtx", "phy", "rez", "ltb", "dtx", "dat", "wav", "mp3", "ogg",
    ];
    const MAX_BYTES: u64 = 1024 * 1024;
    let mut errors = Vec::new();
    for f in tracked_files() {
        let ext = Path::new(&f)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if FORBIDDEN.contains(&ext.as_str()) {
            errors.push(format!("{f}: game asset type .{ext} is tracked. Remove it with `git rm --cached`; assets load at runtime from the user's install."));
        }
        if let Ok(meta) = fs::metadata(root().join(&f))
            && meta.len() > MAX_BYTES
            && f != "Cargo.lock"
        {
            errors.push(format!(
                "{f}: {} bytes. Large files are probably assets or extracted data; keep them out of git.",
                meta.len()
            ));
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// Encryption keys never enter the repo (CLAUDE.md, "Hard rules"): the
/// LithTech loaders, Combat Arms' code and specs hold no hexadecimal run
/// as long as a key or IV (32 digits or more). Keys live in the git-ignored
/// chimera.local.toml and are read at run time.
#[test]
fn no_keys_tracked() {
    const PLACES: &[&str] = &[
        "lithtech/",
        "games/combat_arms/",
        "apps/combat_arms/",
        "apps/mashup_demo/",
        "specs/combat_arms/",
    ];
    let mut errors = Vec::new();
    for f in tracked_files()
        .iter()
        .filter(|f| PLACES.iter().any(|p| f.starts_with(p)))
    {
        let Ok(text) = fs::read_to_string(root().join(f)) else {
            continue;
        };
        let mut run = 0;
        if text.chars().any(|c| {
            run = if c.is_ascii_hexdigit() { run + 1 } else { 0 };
            run >= 32
        }) {
            errors.push(format!(
                "{f}: a run of 32 or more hexadecimal digits, which looks like an encryption key or IV. Keys \
                 never enter the repo: remove it and read keys from chimera.local.toml at run time (tests \
                 skip without them)."
            ));
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// Specs describe behavior; they must not carry source code
/// (specs/README.md, "Rules for spec writers").
#[test]
fn specs_contain_no_code_blocks() {
    let mut errors = Vec::new();
    for f in tracked_files() {
        if !f.starts_with("specs/") || !f.ends_with(".md") || f == "specs/README.md" {
            continue;
        }
        let text = fs::read_to_string(root().join(&f)).unwrap();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("```") && !t.trim_start_matches('`').trim().is_empty() && !t.contains("math") {
                errors.push(format!(
                    "{f}:{}: fenced code block `{t}`. Specs describe behavior as prose, tables and math; \
                     do not include code or pseudocode that mirrors any. Use ```math for formulas.",
                    i + 1
                ));
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// Documents that CLAUDE.md points to must exist, so the map never rots.
#[test]
fn docs_map_links_resolve() {
    let text = fs::read_to_string(root().join("CLAUDE.md")).unwrap();
    let mut errors = Vec::new();
    for part in text.split("](").skip(1) {
        let target = part.split(')').next().unwrap_or("");
        if target.starts_with("http") || target.is_empty() {
            continue;
        }
        let path = target.split('#').next().unwrap();
        if !root().join(path).exists() {
            errors.push(format!(
                "CLAUDE.md links to {target}, which does not exist. Fix the link or restore the file."
            ));
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// AGENTS.md (the contributor contract) and the workflow skills
/// (`.claude/skills/*/SKILL.md`) are what contributors' agents act on, so
/// what they name must exist: their links, the repository scripts they run
/// (`tools/...`, `scripts/...`), and the packages and binaries of their
/// cargo commands (`-p <package>`, `--bin <name>`). Placeholders in angle
/// brackets are skipped.
#[test]
fn agent_docs_commands_exist() {
    let mut packages = BTreeSet::new();
    let mut bins = BTreeSet::new();
    for member in workspace_members() {
        let dir = root().join(&member);
        let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let manifest: toml::Table = text.parse().unwrap();
        if let Some(name) = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            packages.insert(name.to_string());
            bins.insert(name.to_string());
        }
        for bin in manifest.get("bin").and_then(|b| b.as_array()).into_iter().flatten() {
            if let Some(name) = bin.get("name").and_then(|n| n.as_str()) {
                bins.insert(name.to_string());
            }
        }
        if let Ok(entries) = fs::read_dir(dir.join("src/bin")) {
            for e in entries.flatten() {
                let p = e.path();
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if p.extension().is_some_and(|x| x == "rs") || p.join("main.rs").exists() {
                    bins.insert(stem);
                }
            }
        }
    }
    let mut docs = vec![PathBuf::from("AGENTS.md")];
    if let Ok(entries) = fs::read_dir(root().join(".claude/skills")) {
        let mut skills: Vec<PathBuf> = entries
            .flatten()
            .map(|e| PathBuf::from(".claude/skills").join(e.file_name()).join("SKILL.md"))
            .collect();
        skills.sort();
        docs.extend(skills);
    }
    let mut errors = Vec::new();
    for doc in &docs {
        let shown = doc.display().to_string();
        let Ok(text) = fs::read_to_string(root().join(doc)) else {
            errors.push(format!(
                "{shown} is missing: AGENTS.md is the contributor contract and every skill folder holds a SKILL.md."
            ));
            continue;
        };
        let base = root().join(doc).parent().unwrap().to_path_buf();
        for part in text.split("](").skip(1) {
            let target = part.split(')').next().unwrap_or("");
            if target.starts_with("http") || target.is_empty() || target.starts_with('#') {
                continue;
            }
            let path = target.split('#').next().unwrap();
            if !base.join(path).exists() && !root().join(path).exists() {
                errors.push(format!(
                    "{shown} links to {target}, which does not exist. Fix the link or restore the file."
                ));
            }
        }
        for line in text.lines() {
            let cargo = line.contains("cargo ");
            let words: Vec<&str> = line
                .split(|c: char| c.is_whitespace() || "`'\"(),;|".contains(c))
                .filter(|w| !w.is_empty())
                .collect();
            for (i, w) in words.iter().enumerate() {
                let script = w.trim_start_matches(".\\").replace('\\', "/");
                let script = script.trim_end_matches(['.', ':']);
                if (script.starts_with("tools/") || script.starts_with("scripts/"))
                    && !script.contains('<')
                    && !script.contains('*')
                    && !root().join(script).exists()
                {
                    errors.push(format!(
                        "{shown} names {script}, which does not exist. Fix the command or restore the script."
                    ));
                }
                let Some(next) = words.get(i + 1) else { continue };
                if !cargo || next.contains(['<', '>']) || next.starts_with('-') || next.starts_with('$') {
                    continue;
                }
                if *w == "-p" && !packages.contains(*next) {
                    errors.push(format!(
                        "{shown} runs cargo with `-p {next}`, which is not a workspace package. Fix the command \
                     (package names are in each member's Cargo.toml)."
                    ));
                }
                if *w == "--bin" && !bins.contains(*next) {
                    errors.push(format!(
                        "{shown} runs cargo with `--bin {next}`, which no workspace package builds. Fix the command."
                    ));
                }
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// The doc lists built from the sources' `//!` docs (docs/modules.md and the
/// generated entries of docs/tech-debt.md, architecture review T5) match the
/// sources, so features edit only their own file's doc and parallel branches
/// don't conflict on the lists. Also: the hand-kept debt entries stay one
/// short line, and docs/ARCHITECTURE.md points at the module list.
#[test]
fn generated_docs_are_fresh() {
    const HAND_DEBT_LINE: usize = 320;
    let mut errors = Vec::new();
    match repo_checks::docs_index::stale(&root()) {
        Ok(stale) => {
            for (path, _) in stale {
                errors.push(format!(
                    "{path} is stale: it is generated from the files' `//!` docs. Regenerate it \
                     with `{}` and commit the result (change a summary or shortcut in the \
                     file's `//!` doc, never in {path}).",
                    repo_checks::docs_index::COMMAND
                ));
            }
        }
        Err(e) => errors.push(e),
    }
    let debt = fs::read_to_string(root().join("docs/tech-debt.md")).unwrap();
    for line in debt.lines().filter(|l| l.starts_with("- ")) {
        if line.chars().count() > HAND_DEBT_LINE {
            errors.push(format!(
                "docs/tech-debt.md entry longer than {HAND_DEBT_LINE} characters: {line:.80}... \
                 Keep one line; a shortcut that belongs to a file goes in its `//!` doc under \
                 \"Known shortcuts\" and is listed by the generator."
            ));
        }
    }
    let arch = fs::read_to_string(root().join("docs/ARCHITECTURE.md")).unwrap();
    if !arch.contains("docs/modules.md") {
        errors.push("docs/ARCHITECTURE.md must point at docs/modules.md (the generated module list).".into());
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// Binaries built without a test harness (`test = false` in Cargo.toml)
/// must have no unit tests, or those would never run.
#[test]
fn untested_binaries_have_no_tests() {
    let mut errors = Vec::new();
    for member in workspace_members() {
        let dir = root().join(&member);
        let manifest: toml::Table = fs::read_to_string(dir.join("Cargo.toml")).unwrap().parse().unwrap();
        for bin in manifest.get("bin").and_then(|b| b.as_array()).into_iter().flatten() {
            if bin.get("test").and_then(|t| t.as_bool()) != Some(false) {
                continue;
            }
            let name = bin["name"].as_str().unwrap();
            let path = bin
                .get("path")
                .and_then(|p| p.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("src/bin/{name}.rs"));
            let text = fs::read_to_string(dir.join(&path)).unwrap();
            if text.contains("#[test]") || text.contains("#[cfg(test)]") {
                errors.push(format!(
                    "{member}/{path} has tests, but its [[bin]] `{name}` in {member}/Cargo.toml says \
                     `test = false`, so they never run. Remove that `test = false`."
                ));
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// The workspace's members (`[workspace] members` in Cargo.toml), as
/// folders relative to the root.
fn workspace_members() -> Vec<String> {
    let manifest: toml::Table = fs::read_to_string(root().join("Cargo.toml")).unwrap().parse().unwrap();
    manifest["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m.as_str().map(String::from))
        .collect()
}

/// The workspace (docs/ARCHITECTURE.md, "Crates"): every member is a
/// default member, so `cargo test` and `cargo nextest run` at the root (the
/// hooks' commands) test them all; the format crates stay pure data, with
/// no Bevy, physics or game crate under them; the engine's crates
/// depend on no game crate, and nothing on the apps above.
#[test]
fn workspace_crates() {
    let manifest: toml::Table = fs::read_to_string(root().join("Cargo.toml")).unwrap().parse().unwrap();
    let ws = manifest["workspace"].as_table().expect("[workspace] in Cargo.toml");
    let list = |key: &str| -> BTreeSet<String> {
        ws.get(key)
            .and_then(|m| m.as_array())
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_str().map(String::from))
            .collect()
    };
    let (members, defaults) = (list("members"), list("default-members"));
    let mut errors = Vec::new();
    for m in members.difference(&defaults) {
        errors.push(format!(
            "workspace member `{m}` isn't in `default-members` (Cargo.toml), so `cargo test` at the root \
             (and the git hooks) never test it. Add it there."
        ));
    }
    // Every member the hooks can name with `--features dev` needs a `dev`
    // feature, even an empty one: Cargo refuses the whole command if one
    // selected package lacks it. `workspace-hack` is generated by cargo
    // hakari and is never a `-p` test target.
    for m in members.iter().filter(|m| m.as_str() != "chimera/workspace-hack") {
        let path = if m.as_str() == "." {
            "Cargo.toml".to_string()
        } else {
            format!("{m}/Cargo.toml")
        };
        let text = fs::read_to_string(root().join(&path)).unwrap();
        let manifest: toml::Table = text.parse().unwrap();
        if manifest.get("features").and_then(|f| f.get("dev")).is_none() {
            errors.push(format!(
                "{path} has no `dev` feature, so `.githooks/pre-commit`'s `cargo test -p <crate> --features dev` \
                 fails when a change selects it (and the same for the hooks and tools that pass dev). Add an \
                 empty `[features]` entry `dev = []`, as `checks/Cargo.toml` does."
            ));
        }
    }
    // Every member builds third-party crates with the workspace's features
    // (the workspace-hack), so `cargo test -p <crate>` reuses the
    // workspace's builds; but the hack itself, Bevy's development features
    // and the crates that stay free of Bevy (.config/hakari.toml).
    const NO_HACK: &[&str] = &[
        "chimera/workspace-hack",
        "chimera/dev",
        "chimera/vfs",
        "chimera/assets",
        "source/formats",
        "lithtech/formats",
        "source/tools",
        "checks",
    ];
    for m in members.iter().filter(|m| !NO_HACK.contains(&m.as_str())) {
        let path = if m == "." {
            "Cargo.toml".to_string()
        } else {
            format!("{m}/Cargo.toml")
        };
        let text = fs::read_to_string(root().join(&path)).unwrap();
        let manifest: toml::Table = text.parse().unwrap();
        if manifest
            .get("dependencies")
            .and_then(|d| d.get("workspace-hack"))
            .is_none()
        {
            errors.push(format!(
                "{path} doesn't depend on `workspace-hack`, so `cargo test -p` of it builds Bevy and the rest \
                 again with its own features (docs/performance.md, \"Workspace-hack\"). Run `cargo hakari \
                 manage-deps -y` and `cargo hakari generate` (cargo install cargo-hakari)."
            ));
        }
    }
    // Each member's dependencies that may not be (name prefixes), and why.
    const PURE_DATA: &str = "is pure data (docs/ARCHITECTURE.md, \"Crates\"). Keep the parsing there and \
        move what needs it (ECS types, components, conversion to engine types) into the crate that uses \
        it, e.g. `cs_source`.";
    const ENGINE: &str = "is the engine's base (docs/ARCHITECTURE.md, \"Crates\"): games build on it, \
        never the other way. Move the piece that needs the game up into the game or the app (`cs_source_app`), or give the engine a \
        hook (a registry, a trait, a resource) that the game fills.";
    const WORLD: &str = "is the simulation's map (docs/ARCHITECTURE.md, \"Crates\"): what draws and \
        plays it (`chimera_present`) and the games sit above it. Say what is needed in plain data (a \
        drawn-part request, a `MapDrawer` or `MapHooks` hook) and let the crate above fill it in.";
    const PRESENT: &str = "is the engine's presentation (docs/ARCHITECTURE.md, \"Crates\"): it draws \
        and plays the map, and games build on it. Move the piece that needs the game up into the game or the app (`cs_source_app`), or \
        give the presentation a hook (a registry, a drawn-part request, a resource) that the game fills.";
    const SIM: &str = "is the simulation's frameworks (docs/ARCHITECTURE.md, \"Crates\"): the games \
        fill them and sit above them. Move the piece that needs the game up into the game or the app (`cs_source_app`), or give the \
        framework a hook (a registry, a trait, data) that the game fills.";
    const NET: &str = "is the engine's network (docs/ARCHITECTURE.md, \"Crates\"): Source's logic \
        and the games register what they replicate there (`net::NetAppExt::net_part`) and fill its hooks \
        (`net::parts`); it never names them, and a dedicated server runs it without drawing. Give the \
        network a hook (a part, a resource of fn pointers) that the crate above fills.";
    const GAME: &str = "is a game's simulation (docs/ARCHITECTURE.md, \"Crates\"): it builds on the \
        engine and runs on a dedicated server without drawing; its front end and the apps sit above it, \
        and games never use each other. Move the piece into its front end (`ui`, whose feature alone may \
        turn on the presentation and the shell: UI_FEATURES) or up into the app (`cs_source_app`), or give the engine a hook \
        the game fills.";
    const SOURCE: &str = "is Source's shared behaviour (docs/ARCHITECTURE.md, \"Crates\"): it sits \
        on the engine's simulation, and what presents it and the games sit above it. Say what is needed in \
        plain data or give it a hook (a registry, a resource) that the crate above fills.";
    const CLIENT: &str = "is the client shell (docs/ARCHITECTURE.md, \"Crates\"): it sits on the engine's \
        crates (the presentation and the network too) below the games' front ends and the app, and knows \
        nothing of a game, the app or Source's logic. Give the shell a hook (a resource such as \
        `client::AppHooks` or `debug_ui::GameDebug`, data in `core` or `map::logic_view`) that the crate \
        above fills.";
    const APP: &str = "is the CS:S app's glue (docs/ARCHITECTURE.md, \"Crates\"): the dedicated server, the \
        tools and the integration tests (`mashup`) sit above it. Give it what it needs as a hook or move \
        the piece into the crate that needs it.";
    const CA_APP: &str = "is the Combat Arms app's glue (docs/ARCHITECTURE.md, \"Crates\"): Chimera and Combat \
        Arms composed into its game. It never uses CS:S or Source's logic: what both games need moves into \
        Chimera (a lift), not across.";
    const MASHUP_APP: &str = "is a mashup app (docs/ARCHITECTURE.md, \"Crates\"): it composes games as \
        libraries (`cs_source`, `combat_arms`) on Chimera, never another app: what it shares with a game's \
        own app moves into the game or Chimera (a map loader, a dressing, a hook), not across.";
    const ORIGINALS: &str = "is Lucker Party's original minigames (docs/ARCHITECTURE.md, \"Crates\"; \
        architecture-review.md A5): Chimera only. A game's parts reach a minigame through Chimera's slots and \
        registries (a weapon by id from the catalogue, a movement by id), filled by the app that composes the \
        games, never through a game crate or an app.";
    const PARTY_KIT: &str = "is the party kit (docs/ARCHITECTURE.md, \"Crates\"): the games as libraries, \
        the original minigames and Chimera composed once for the four party apps (Lucker Party, the Ultimate \
        Gun Game, and their servers), never one of those apps: what they share lives here, not across them.";
    const PARTY_APP: &str = "is Lucker Party's game binary (docs/ARCHITECTURE.md, \"Crates\"): a thin binary \
        over the party kit (`party_kit`), never another app and no game crate: what it needs from the games \
        goes through the kit.";
    const PARTY_SERVER: &str = "is Lucker Party's dedicated server (docs/ARCHITECTURE.md, \"Crates\"): \
        a thin binary over the party kit (`party_kit`) without its default `present`, so no presentation or \
        client shell links (`server_builds_without_presentation`). Move what needs them into the kit's \
        client side (`present`).";
    const UGG_APP: &str = "is the Ultimate Gun Game (docs/ARCHITECTURE.md, \"Crates\"; \
        docs/plans/active/ultimate-gun-game.md): the party kit (`party_kit`) with an automatic gun game made \
        from the games' parts (`chimera_core::game_parts`). It names no game crate and no other app: what a \
        game lends reaches it through the parts registry, and what it shares with the party lives in the kit.";
    const UGG_SERVER: &str = "is the Ultimate Gun Game's dedicated server (docs/ARCHITECTURE.md, \"Crates\"): \
        the party kit (`party_kit::server`) and `ultimate_gun_game` without their default `present`, so no \
        presentation or client shell links (`server_builds_without_presentation`).";
    const SERVER: &str = "is the dedicated server (docs/ARCHITECTURE.md, \"Crates\"): it builds without the \
        presentation and the client shell (`server_builds_without_presentation`). Move what needs them \
        into the client's side or behind `cs_source_app`'s `present`.";
    const CA_SERVER: &str = "is Combat Arms' dedicated server (docs/ARCHITECTURE.md, \"Crates\"): Chimera's \
        simulation crates and Combat Arms without its front end (`ui`), no presentation, client shell or \
        Combat Arms app (`server_builds_without_presentation`), and never CS:S or Source's logic. Move what \
        needs them into the client's side (the app, `ui`).";
    const TOOLS: &str = "is a layer's tools (docs/ARCHITECTURE.md, \"Crates\"): a tool lives at the lowest \
        layer whose knowledge it needs, and uses nothing above it. Move the tool (or the part of it that \
        needs the layer above) into that layer's tools: chimera/tools, source/tools, games/<game>/tools.";
    const CHECKS: &str = "is the repository's checks (docs/ARCHITECTURE.md, \"Crates\"): plain crates only, so \
        the architecture tests and the hooks' test selection build in seconds in a fresh clone and never \
        depend on the code they check. Read what is needed from files or `cargo metadata` instead.";
    const FORBIDDEN: &[(&str, &[&str], &str)] = &[
        // The neutral VFS under both format crates: no dependency at all.
        (
            "chimera/vfs",
            &[
                "bevy",
                "avian",
                "mashup",
                "source_",
                "lithtech_",
                "chimera_",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
                "workspace-hack",
            ],
            PURE_DATA,
        ),
        // The asset store: plain data over the VFS (`chimera_vfs` only),
        // under Chimera's `mount`.
        (
            "chimera/assets",
            &[
                "bevy",
                "avian",
                "mashup",
                "source_",
                "lithtech_",
                "chimera_core",
                "chimera_world",
                "chimera_present",
                "chimera_sim",
                "chimera_net",
                "chimera_client",
                "chimera_tools",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
                "workspace-hack",
            ],
            PURE_DATA,
        ),
        // Source's formats may use the VFS (`chimera_vfs`) and no other
        // Chimera crate.
        ("source/formats", &["bevy", "avian", "mashup", "lithtech_", "chimera_"], PURE_DATA),
        // LithTech's loader layer, like Source's: plain data (the VFS from
        // `chimera_vfs` only).
        (
            "lithtech/formats",
            &[
                "bevy",
                "avian",
                "mashup",
                "chimera_",
                "source_logic",
                "source_tools",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
                "workspace-hack",
            ],
            PURE_DATA,
        ),
        ("chimera/core", &["mashup", "cs_source", "combat_arms", "games", "originals", "lucker_party", "party_kit"], ENGINE),
        (
            "chimera/world",
            &["chimera_present", "mashup", "cs_source", "combat_arms", "games", "originals", "lucker_party", "party_kit"],
            WORLD,
        ),
        (
            "chimera/present",
            &["mashup", "cs_source", "combat_arms", "games", "originals", "lucker_party", "party_kit"],
            PRESENT,
        ),
        (
            "chimera/sim",
            &["chimera_present", "mashup", "cs_source", "combat_arms", "games", "originals", "lucker_party", "party_kit"],
            SIM,
        ),
        (
            "chimera/net",
            &[
                "chimera_present",
                "source_logic",
                "mashup",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
            ],
            NET,
        ),
        (
            "games/cs_source",
            &[
                "chimera_present",
                "chimera_client",
                "cs_source_app",
                "mashup",
                "combat_arms",
                "originals",
                "lucker_party",
                "party_kit",
            ],
            GAME,
        ),
        (
            "games/combat_arms",
            &[
                "chimera_present",
                "chimera_client",
                "source_logic",
                "cs_source",
                "combat_arms_app",
                "mashup",
                "originals",
                "lucker_party",
                "party_kit",
            ],
            GAME,
        ),
        (
            "source/logic",
            &["chimera_present", "mashup", "cs_source", "combat_arms", "games", "originals", "lucker_party", "party_kit"],
            SOURCE,
        ),
        ("apps/cs_source", &["mashup", "combat_arms"], APP),
        ("apps/combat_arms", &["mashup", "cs_source", "source_logic"], CA_APP),
        (
            "apps/mashup_demo",
            &["mashup", "cs_source_app", "combat_arms_app", "lucker_party", "party_kit"],
            MASHUP_APP,
        ),
        (
            "games/originals",
            &[
                "chimera_present",
                "chimera_client",
                "source_",
                "lithtech_",
                "cs_source",
                "combat_arms",
                "mashup",
                "lucker_party",
                "party_kit",
            ],
            ORIGINALS,
        ),
        (
            "apps/party_kit",
            &["mashup", "cs_source_app", "combat_arms_app", "ultimate_gun_game", "lucker_party"],
            PARTY_KIT,
        ),
        (
            "apps/lucker_party",
            &[
                "mashup",
                "cs_source",
                "source_logic",
                "combat_arms",
                "originals",
                "ultimate_gun_game",
                "lucker_party_server",
            ],
            PARTY_APP,
        ),
        (
            "apps/lucker_party_server",
            &[
                "chimera_present",
                "chimera_client",
                "bevy_egui",
                "bevy-inspector-egui",
                "arboard",
                "cs_source",
                "combat_arms",
                "mashup",
                "ultimate_gun_game",
                "lucker_party",
            ],
            PARTY_SERVER,
        ),
        (
            "apps/ultimate_gun_game",
            &["mashup", "cs_source", "source_logic", "cs_source_app", "combat_arms_app", "lucker_party"],
            UGG_APP,
        ),
        (
            "apps/ultimate_gun_game_server",
            &[
                "chimera_present",
                "chimera_client",
                "bevy_egui",
                "bevy-inspector-egui",
                "arboard",
                "cs_source",
                "combat_arms",
                "mashup",
                "lucker_party",
            ],
            UGG_SERVER,
        ),
        (
            "chimera/tools",
            &["source_", "cs_source", "combat_arms", "games", "mashup"],
            TOOLS,
        ),
        (
            "source/tools",
            &[
                "bevy",
                "avian",
                "chimera_",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
                "mashup",
            ],
            TOOLS,
        ),
        (
            "checks",
            &[
                "bevy",
                "avian",
                "chimera_",
                "source_",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
                "mashup",
                "workspace-hack",
            ],
            CHECKS,
        ),
        ("games/cs_source/tools", &["mashup", "combat_arms"], TOOLS),
        (
            "apps/cs_source_server",
            &[
                "chimera_present",
                "chimera_client",
                "bevy_egui",
                "bevy-inspector-egui",
                "arboard",
                "mashup",
            ],
            SERVER,
        ),
        (
            "apps/combat_arms_server",
            &[
                "chimera_present",
                "chimera_client",
                "bevy_egui",
                "bevy-inspector-egui",
                "arboard",
                "combat_arms_app",
                "cs_source",
                "source_logic",
                "mashup",
            ],
            CA_SERVER,
        ),
        (
            "chimera/client",
            &[
                "source_logic",
                "bevy_replicon",
                "mashup",
                "cs_source",
                "combat_arms",
                "games",
                "originals",
                "lucker_party",
                "party_kit",
            ],
            CLIENT,
        ),
    ];
    for (krate, forbidden, why) in FORBIDDEN {
        let path = format!("{krate}/Cargo.toml");
        let text = fs::read_to_string(root().join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let m: toml::Table = text.parse().unwrap();
        // A game's front end (`UI_FEATURES`) may use the presentation, the
        // shell and Source's logic: as optional dependencies only its
        // feature turns on.
        let ui_only = |dep: &str| {
            UI_FEATURES.iter().any(|(k, feature, deps)| {
                k == krate
                    && deps.contains(&dep)
                    && m.get("dependencies")
                        .and_then(|d| d.get(dep))
                        .and_then(|d| d.get("optional"))
                        .and_then(|o| o.as_bool())
                        == Some(true)
                    && m.get("features").and_then(|f| f.as_table()).is_some_and(|features| {
                        let turns_on = |list: &toml::Value| {
                            list.as_array().into_iter().flatten().any(|v| {
                                v.as_str().is_some_and(|v| {
                                    v == format!("dep:{dep}") || v == dep || v.starts_with(&format!("{dep}/"))
                                })
                            })
                        };
                        features.iter().all(|(name, list)| name == feature || !turns_on(list))
                    })
            })
        };
        let mut tables: Vec<(&str, &toml::Table)> = ["dependencies", "dev-dependencies", "build-dependencies"]
            .iter()
            .filter_map(|k| m.get(*k).and_then(|t| t.as_table()).map(|t| (*k, t)))
            .collect();
        for target in m
            .get("target")
            .and_then(|t| t.as_table())
            .into_iter()
            .flat_map(|t| t.values())
        {
            tables.extend(
                ["dependencies", "dev-dependencies"]
                    .iter()
                    .filter_map(|k| target.get(*k).and_then(|t| t.as_table()).map(|t| (*k, t))),
            );
        }
        // A test-only dependency (`TEST_ONLY`): in dev-dependencies, and
        // nowhere else but as the front end's optional one.
        let test_only = |dep: &str| {
            TEST_ONLY.iter().any(|(k, deps, _)| k == krate && deps.contains(&dep))
                && ["dependencies", "build-dependencies"].iter().all(|t| {
                    m.get(*t)
                        .and_then(|d| d.get(dep))
                        .is_none_or(|d| d.get("optional").and_then(|o| o.as_bool()) == Some(true))
                })
        };
        for (table, dep) in tables.iter().flat_map(|(name, t)| t.keys().map(move |k| (*name, k))) {
            // The front end's optional dependencies in the build's tables;
            // the test-only ones in the tests' (dev-dependencies).
            let allowed = if table == "dev-dependencies" {
                test_only(dep)
            } else {
                ui_only(dep)
            };
            // `chimera_vfs` is the neutral layer under the format crates, so
            // the `chimera_` prefix in their rules doesn't cover it (its own
            // rules do list the whole prefix, and it has no dependencies).
            let neutral = dep == "chimera_vfs" && *krate != "chimera/vfs";
            if forbidden.iter().any(|f| dep.starts_with(f) && !(neutral && *f == "chimera_")) && !allowed {
                errors.push(format!("{path} depends on `{dep}`: {krate} {why}"));
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// A game crate's front end feature and the dependencies only it may
/// turn on (optional ones): the presentation and the client shell, which
/// the game's simulation never uses (`workspace_crates`;
/// `game_ui_only_with_its_feature` checks the dependency tree).
const UI_FEATURES: &[(&str, &str, &[&str])] = &[
    ("games/cs_source", "ui", &["chimera_present", "chimera_client"]),
    ("games/combat_arms", "ui", &["chimera_present", "chimera_client"]),
];

/// Crates a member's `FORBIDDEN` list names that its tests alone may use
/// (dev-dependencies, never a normal or build one): crates below it, not
/// above, so it is no way around the layering. `cs_source`'s scenario
/// tests run the map as the game does, its drawn parts and sounds with it
/// (`chimera_present`'s `MapPlugin`); the game's own build never links it
/// (`game_ui_only_with_its_feature`).
const TEST_ONLY: &[(&str, &[&str], &str)] = &[(
    "games/cs_source",
    &["chimera_present"],
    "the scenario tests' maps with what draws and plays them",
)];

/// A game crate builds without its front end: `cs_source` alone depends
/// on no presentation or shell; with `ui` it does (`UI_FEATURES`). The dedicated server never turns `ui` on
/// (`server_builds_without_presentation`).
#[test]
fn game_ui_only_with_its_feature() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    for (krate, feature, deps) in UI_FEATURES {
        let package = krate.rsplit('/').next().unwrap();
        let tree = |args: &[&str]| -> String {
            let out = Command::new(&cargo)
                .current_dir(root())
                .args([
                    "tree",
                    "-p",
                    package,
                    "-e",
                    "normal",
                    "--prefix",
                    "none",
                    "--offline",
                    "--locked",
                ])
                .args(args)
                .output()
                .expect("running cargo tree");
            assert!(
                out.status.success(),
                "cargo tree failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).into_owned()
        };
        let has = |tree: &str, dep: &str| tree.lines().any(|l| l.starts_with(&format!("{dep} ")));
        let bare = tree(&[]);
        let present: Vec<&&str> = deps.iter().filter(|d| has(&bare, d)).collect();
        assert!(
            present.is_empty(),
            "`{package}` without its `{feature}` feature depends on {present:?}: only its front end may. Make the \
             dependency optional and turn it on in `{feature}` ({krate}/Cargo.toml), or move what needs it into \
             the front end (`ui`)."
        );
        let with_ui = tree(&["--features", feature]);
        assert!(
            deps.iter().any(|d| has(&with_ui, d)),
            "`{package}` with `{feature}` no longer depends on any of {deps:?}: update UI_FEATURES in \
             checks/tests/it/architecture.rs and docs/ARCHITECTURE.md (\"Crates\")."
        );
    }
}

/// The dedicated servers build without the presentation
/// (docs/ARCHITECTURE.md, "Crates"): `cs_source_server` uses the app's
/// glue (`cs_source_app`) without its default feature `present`, and
/// `combat_arms_server` Chimera's simulation crates and Combat Arms
/// without `ui`, and `lucker_party_server` the party kit
/// (`party_kit`) without its `present`, so none depends on
/// `chimera_present` or links render or audio code. (That they compile that way the pre-push hook checks:
/// `cargo check -p cs_source_server`, `-p combat_arms_server`.)
#[test]
fn server_builds_without_presentation() {
    // The presentation, crates only the client shell uses, and Bevy's own
    // render, window and audio crates (the workspace's `bevy` has no default
    // features; `chimera_present` and `chimera_client` ask for them, A5).
    const CLIENT_ONLY: &[&str] = &[
        "chimera_present",
        "chimera_client",
        "bevy_egui",
        "bevy-inspector-egui",
        "arboard",
        "bevy_render",
        "bevy_pbr",
        "bevy_core_pipeline",
        "bevy_ui",
        "bevy_winit",
        "bevy_audio",
        "bevy_gltf",
        "wgpu",
        "wgpu-core",
        "winit",
        "rodio",
    ];
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let tree = |package: &str| -> String {
        let out = Command::new(&cargo)
            .current_dir(root())
            .args([
                "tree",
                "-p",
                package,
                "-e",
                "normal",
                "--prefix",
                "none",
                "--offline",
                "--locked",
            ])
            .output()
            .expect("running cargo tree");
        assert!(
            out.status.success(),
            "cargo tree failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let uses_present = |tree: &str| tree.lines().any(|l| l.starts_with("chimera_present "));
    assert!(
        uses_present(&tree("cs_source_app")),
        "the game (`cs_source_app` with its default features) no longer depends on chimera_present: update this \
         test and docs/ARCHITECTURE.md (\"Crates\")."
    );
    let server = tree("cs_source_server");
    let present: Vec<&str> = server
        .lines()
        .filter(|l| CLIENT_ONLY.iter().any(|c| l.starts_with(&format!("{c} "))))
        .collect();
    assert!(
        present.is_empty(),
        "`cs_source_server` (the dedicated server's build) depends on {present:?}. The presentation \
         (chimera_present) and the client-only crates are optional in `cs_source_app`, behind its default \
         feature `present` (apps/cs_source/Cargo.toml): make the new dependency optional and add it to \
         `present`, or move what the simulation needs out of the presentation."
    );
    let server = tree("combat_arms_server");
    let present: Vec<&str> = server
        .lines()
        .filter(|l| CLIENT_ONLY.iter().any(|c| l.starts_with(&format!("{c} "))))
        .collect();
    assert!(
        present.is_empty(),
        "`combat_arms_server` (the Combat Arms dedicated server's build) depends on {present:?}. It uses \
         Chimera's simulation crates and `combat_arms` without its `ui` feature (apps/combat_arms_server/\
         Cargo.toml): don't turn on `ui` or depend on the app (`combat_arms_app`), and keep the new \
         dependency out of the simulation crates (the front end's optional ones: UI_FEATURES)."
    );
    let server = tree("lucker_party_server");
    let present: Vec<&str> = server
        .lines()
        .filter(|l| CLIENT_ONLY.iter().any(|c| l.starts_with(&format!("{c} "))))
        .collect();
    assert!(
        present.is_empty(),
        "`lucker_party_server` (Lucker Party's dedicated server's build) depends on {present:?}. It uses \
         `party_kit` without its default feature `present` (the workspace's dependency, Cargo.toml) and \
         the games without their front ends: make the new dependency optional in `party_kit` behind \
         `present`, or move what the simulation needs out of the presentation."
    );
    let server = tree("ultimate_gun_game_server");
    let present: Vec<&str> = server
        .lines()
        .filter(|l| CLIENT_ONLY.iter().any(|c| l.starts_with(&format!("{c} "))))
        .collect();
    assert!(
        present.is_empty(),
        "`ultimate_gun_game_server` depends on {present:?}. It uses `party_kit` and \
         `ultimate_gun_game` without their default feature `present` (the workspace's dependency, Cargo.toml): \
         make the new dependency optional in `ultimate_gun_game` behind `present`."
    );
}

/// Each package's integration tests are one test binary per crate
/// (`tests/it/main.rs`, `[[test]] name = "it"`), plus binaries its
/// manifest declares for a feature (`cs_source`'s `ui`, tests/ui/): every
/// other file directly in a `tests/` folder would be a binary of its own,
/// linking the crate and everything under it again (docs/performance.md,
/// "Test cycle"). A file in a binary's folder without its `mod` line is
/// never compiled. Folders without a `main.rs` (`tests/common/`) are
/// helpers the binaries share.
#[test]
fn tests_are_one_binary_per_crate() {
    let mut errors = Vec::new();
    for member in workspace_members() {
        let dir = if member == "." { root() } else { root().join(&member) };
        let tests = dir.join("tests");
        if !tests.is_dir() {
            continue;
        }
        let shown = |p: &Path| p.strip_prefix(root()).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let manifest: toml::Table = fs::read_to_string(dir.join("Cargo.toml")).unwrap().parse().unwrap();
        let declared: Vec<String> = manifest
            .get("test")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter_map(|t| t.get("path").and_then(|p| p.as_str()).map(String::from))
            .collect();
        for entry in fs::read_dir(&tests).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let own_binary = if path.is_dir() {
                path.join("main.rs").exists() && !declared.contains(&format!("tests/{name}/main.rs"))
            } else {
                name.ends_with(".rs")
            };
            if own_binary {
                let stem = name.trim_end_matches(".rs");
                errors.push(format!(
                    "{}: a test binary of its own, which links the crate again. Move it into the crate's one \
                     test binary, tests/it/ (tests/it/heavy/ if it loads real maps or simulates for long: the \
                     fast tier skips those), with `mod {stem};` in tests/it/main.rs and `[[test]] name = \"it\"` \
                     in {member}/Cargo.toml; or, for tests that need a feature, declare the binary there \
                     (`[[test]]` with `required-features`).",
                    shown(&path)
                ));
            }
        }
        for main in &declared {
            let main = dir.join(main);
            let Some(bin_dir) = main
                .parent()
                .filter(|_| main.file_name().is_some_and(|n| n == "main.rs"))
            else {
                continue;
            };
            let main_text = fs::read_to_string(&main).unwrap_or_default();
            let mut files = Vec::new();
            rust_files(bin_dir, &mut files);
            for file in files {
                let rel = file.strip_prefix(bin_dir).unwrap().to_path_buf();
                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                if rel == Path::new("main.rs") || stem == "mod" {
                    continue;
                }
                // The module that declares it: main.rs for top-level files
                // and inline blocks (`mod heavy { mod x; }`), else its
                // folder's `mod.rs` or `<folder>.rs`.
                let parent = rel
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(Path::to_path_buf);
                let text = match &parent {
                    None => main_text.clone(),
                    Some(p) => {
                        let folder = p.file_name().unwrap().to_string_lossy();
                        let inline = format!("mod {folder} {{");
                        match main_text.split_once(&inline) {
                            Some((_, rest)) if p.components().count() == 1 => rest.to_string(),
                            _ => fs::read_to_string(bin_dir.join(p).join("mod.rs"))
                                .or_else(|_| fs::read_to_string(bin_dir.join(p).with_extension("rs")))
                                .unwrap_or_default(),
                        }
                    }
                };
                if !text.lines().any(|l| l.trim() == format!("mod {stem};")) {
                    errors.push(format!(
                        "{}: never compiled: add `mod {stem};` to {}{}.",
                        shown(&file),
                        shown(&main),
                        parent
                            .as_ref()
                            .map_or(String::new(), |p| format!(" (inside `mod {} {{ }}`)", p.display()))
                    ));
                }
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}
