//! ECS-style checks (docs/plans/active/ecs-cleanup.md, step 1; DESIGN.md,
//! "ECS style"): games plug in through components, observers, messages
//! and systems with declared queries, and whole-world access stays where
//! the job is the world.
//!
//! - `callback_registries_are_allowed`: every struct, enum or type alias
//!   that stores a callback taking the world (`fn(&mut World, ..)`,
//!   `dyn Fn(&World, ..)`, `DeferredWorld`, `EntityWorldMut`) is on
//!   `ALLOWED_REGISTRIES` with its reason; an entry that no longer exists
//!   fails too.
//! - `keyed_hooks_are_not_single_slot_resources`: the hook kinds keyed by
//!   game (`KEYED_HOOKS`: `chimera_core::gameplay::GameHooks`, plan step
//!   2) are never inserted or read as a resource of their own, which the
//!   last plugin added would replace.
//! - `per_tick_exclusive_systems_ratchet`: per crate, the functions taking
//!   `&mut World` that an `add_systems` call puts in a schedule other than
//!   the startup ones and `OnEnter`/`OnExit`/`OnTransition`, outside the
//!   files on `ALLOWED_EXCLUSIVE_FILES`. The stored counts
//!   (`EXCLUSIVE_BASELINE`) only fall, like `determinism.rs`'s; a crate
//!   not listed is held at 0.
//!
//! Text scans, fast and approximate in the same way as the plan's
//! inventory (section 1): comments and string literals are stripped, a
//! file is cut at a column-0 `#[cfg(test)]`, and `tests.rs`, `tests/`,
//! `ui` folders and developer-tool crates are skipped. A system named by
//! a plain name matches a function of that name in the same file; a path
//! (`flow::update`) matches a function in a module of the same crate
//! whose path ends with it. Systems from other crates or behind `use`
//! aren't seen.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use super::architecture::{root, rust_files};

/// Folders under the repository root holding crates (`<group>/<crate>`).
const GROUPS: &[&str] = &["chimera", "source", "lithtech", "games", "apps"];

/// Crates not scanned: developer tools and generated code.
const NOT_SCANNED: &[&str] = &["chimera/dev", "chimera/tools", "chimera/workspace-hack", "source/tools"];

/// Schedules that run once (or on a state change), not every tick or
/// frame.
const ONE_OFF_SCHEDULES: &[&str] = &[
    "Startup",
    "PreStartup",
    "PostStartup",
    "RenderStartup",
    "OnEnter",
    "OnExit",
    "OnTransition",
];

/// Types that store whole-world callbacks, by file and type, with the
/// reason the job needs the world. "S" rows are single-slot hooks to be
/// keyed by game (plan step 3; step 2's are keyed: `KEYED_HOOKS`);
/// "B"/"C" fields are to become
/// events, observers or systems (the step named).
const ALLOWED_REGISTRIES: &[(&str, &str, &str)] = &[
    (
        "chimera/client/src/client/debug_ui.rs",
        "GameDebug",
        "a tool: the debug window's game rows",
    ),
    (
        "chimera/client/src/client/front_end.rs",
        "FrontEnd",
        "the shell's one-off transitions to the menus",
    ),
    (
        "chimera/client/src/client/mod.rs",
        "AppHooks",
        "the app's map loading and greybox",
    ),
    (
        "chimera/core/src/console.rs",
        "CompleteFn",
        "the console: a command completes from anything",
    ),
    (
        "chimera/core/src/console.rs",
        "GetFn",
        "the console: a cvar reads anything",
    ),
    (
        "chimera/core/src/console.rs",
        "RunFn",
        "the console: a command does anything",
    ),
    (
        "chimera/core/src/console.rs",
        "SetFn",
        "the console: a cvar writes anything",
    ),
    (
        "chimera/core/src/core.rs",
        "NetCodec",
        "a network codec, generic per type",
    ),
    (
        "chimera/core/src/core.rs",
        "PredictedComponent",
        "prediction's save and restore, generic per type",
    ),
    (
        "chimera/core/src/game_log.rs",
        "LogSubjects",
        "names in a log line, keyed by game (`gameplay::GameHooks`)",
    ),
    (
        "chimera/core/src/gameplay.rs",
        "NewGame",
        "map start: the rules' fresh game, keyed by game (`gameplay::GameHooks`)",
    ),
    (
        "chimera/core/src/slots.rs",
        "MovementImpl",
        "a movement's insert and remove on its entity, keyed by id",
    ),
    (
        "chimera/net/src/minigame_net.rs",
        "NextMatchHooks",
        "the match change: map loading and spawning",
    ),
    (
        "chimera/net/src/net/parts.rs",
        "BotHooks",
        "the bots' quota and kicks: spawning, keyed by game (`gameplay::GameHooks`)",
    ),
    (
        "chimera/net/src/net/parts.rs",
        "Holds",
        "the rules' freeze for net capture and prediction, keyed by game (`gameplay::GameHooks`)",
    ),
    (
        "chimera/net/src/net/parts.rs",
        "JoinRules",
        "a join spawns and places the player, keyed by game (`gameplay::GameHooks`)",
    ),
    (
        "chimera/net/src/net/parts.rs",
        "NetHooks",
        "serving's one-off steps; `replay` inside prediction's replay loop",
    ),
    (
        "chimera/net/src/net/parts.rs",
        "OwnPart",
        "a network codec, sorted by name",
    ),
    (
        "chimera/net/src/net/voice.rs",
        "VoiceRules",
        "per voice packet, the app's rule",
    ),
    (
        "chimera/net/src/net/vote.rs",
        "VoteIssue",
        "a vote's check and run: console work",
    ),
    (
        "chimera/sim/src/minigame.rs",
        "Modifier",
        "applied once at a minigame's start",
    ),
    (
        "chimera/sim/src/minigame.rs",
        "RulesModule",
        "a rules module's start and kit (a join and a life's arming are events: `Joined`, `Arming`)",
    ),
    (
        "chimera/sim/src/minigame/ladder.rs",
        "RungExtrasHook",
        "a rung's seeded draw and arming, before `OnRung`",
    ),
    (
        "chimera/sim/src/mode.rs",
        "Flow",
        "the flow's respawn and length; `players` to a kept state, `event` triggered as an event (step 9)",
    ),
    (
        "chimera/sim/src/mode.rs",
        "Hooks",
        "a mode's steps in the flow's order (its per-tick work is systems gated by `mode_is`, a death's reaction an observer of `ModeDeath`)",
    ),
    (
        "chimera/sim/src/mode.rs",
        "Mode",
        "a mode's rules at counted deaths and round ends, ordered by the flow",
    ),
    (
        "chimera/sim/src/mode.rs",
        "ModeGame",
        "which mode a game chose, per step",
    ),
    (
        "chimera/sim/src/rule_module.rs",
        "RuleModule",
        "whether a module applies: map load, cvars",
    ),
    (
        "chimera/sim/src/server_docs.rs",
        "DocApply",
        "applying a server document: rebuilds data in place",
    ),
    (
        "chimera/sim/src/weapon/data.rs",
        "HookBuild",
        "a weapon's build inserts its parts on the weapon entity",
    ),
    (
        "chimera/sim/src/weapon/economy.rs",
        "ItemPart",
        "buying an item gives its part: spawning",
    ),
    (
        "chimera/sim/src/weapon/mod.rs",
        "BuildWeapon",
        "building a weapon entity: spawning",
    ),
    (
        "chimera/sim/src/weapon/params.rs",
        "ParamCodec",
        "a network codec of weapon parameters",
    ),
    (
        "chimera/sim/src/weapon/sync.rs",
        "PartCodec",
        "a network codec, generic per component type",
    ),
    ("chimera/world/src/map/hazards.rs", "SpawnHazards", "map loading"),
    (
        "chimera/world/src/map/logic_view.rs",
        "LogicInspect",
        "a tool: the map-logic inspector",
    ),
    ("chimera/world/src/map/mod.rs", "MapHooks", "map loading and unloading"),
    (
        "chimera/world/src/map/mod.rs",
        "MapScopedResource",
        "map unloading removes the map's resources",
    ),
    (
        "games/combat_arms/src/game/rules.rs",
        "CaMode",
        "B: a player leaving; to the `on_leave` observer (step 8)",
    ),
];

/// Files whose per-tick exclusive systems do whole-world jobs (network
/// apply and capture, map loading, the console, menus, IO, demo playback,
/// the map-logic interpreter, servers' state changes), with the reason.
/// Their systems don't count in `EXCLUSIVE_BASELINE`.
const ALLOWED_EXCLUSIVE_FILES: &[(&str, &str)] = &[
    ("apps/combat_arms_server/src/lib.rs", "the server's level changes"),
    ("apps/combat_arms_server/src/main.rs", "the server's state changes"),
    (
        "apps/cs_source_server/src/main.rs",
        "the server's level and state changes",
    ),
    ("apps/party_kit/src/controls.rs", "a menu"),
    ("apps/party_kit/src/esc_menu.rs", "a menu"),
    ("apps/party_kit/src/server.rs", "the server's level and state changes"),
    ("chimera/client/src/client/console.rs", "the console and map loading"),
    ("chimera/client/src/client/debug_ui.rs", "a tool"),
    ("chimera/client/src/client/net.rs", "the connection's state"),
    ("chimera/core/src/console.rs", "the console"),
    ("chimera/core/src/game_log.rs", "writing the log (IO)"),
    ("chimera/net/src/minigame_net.rs", "network apply and capture"),
    ("chimera/net/src/net/chat.rs", "network apply and capture"),
    ("chimera/net/src/net/client.rs", "network apply and capture"),
    ("chimera/net/src/net/cvars.rs", "network apply and capture"),
    ("chimera/net/src/net/game.rs", "network apply and capture"),
    ("chimera/net/src/net/maps.rs", "network apply and capture, map loading"),
    (
        "chimera/net/src/net/predict.rs",
        "prediction's generic save and restore",
    ),
    ("chimera/net/src/net/server.rs", "network capture"),
    ("chimera/net/src/net/server_docs.rs", "network apply and capture"),
    ("chimera/net/src/net/upnp.rs", "network IO"),
    ("chimera/net/src/net/vote.rs", "votes: console work"),
    ("chimera/net/src/net/weapon_params.rs", "network apply and capture"),
    ("games/combat_arms/src/game/chat.rs", "messaging IO"),
    ("games/combat_arms/src/game/lobby/host.rs", "the lobby's state machine"),
    ("games/combat_arms/src/game/net.rs", "network apply and capture"),
    ("games/combat_arms/src/game/objects/net.rs", "network apply and capture"),
    ("games/combat_arms/src/game/options/flow.rs", "saving options (IO)"),
    ("games/combat_arms/src/game/radar/mod.rs", "network capture"),
    ("games/cs_source/src/game/demo.rs", "demo playback"),
    ("games/cs_source/src/game/net.rs", "network apply and capture"),
    ("games/cs_source/src/harness.rs", "map loading"),
    ("source/logic/src/logic/bridge.rs", "the map-logic interpreter's I/O"),
    ("source/logic/src/logic/net.rs", "network apply and capture"),
];

/// Per crate (`<group>/<crate>`): exclusive systems in per-tick or
/// per-frame schedules outside `ALLOWED_EXCLUSIVE_FILES`. Lower a number
/// when you remove one; never raise one.
const EXCLUSIVE_BASELINE: &[(&str, usize)] = &[
    ("chimera/client", 1),
    ("chimera/sim", 5),
    ("chimera/world", 1),
    ("games/combat_arms", 17),
    ("games/cs_source", 4),
];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every scanned crate (`<group>/<crate>`), found on disk, sorted.
fn crates() -> Vec<String> {
    let mut out = Vec::new();
    for group in GROUPS {
        let Ok(entries) = fs::read_dir(root().join(group)) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let name = format!("{group}/{}", path.file_name().unwrap().to_string_lossy());
            if path.join("Cargo.toml").is_file() && path.join("src").is_dir() && !NOT_SCANNED.contains(&name.as_str()) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

/// Source without comments, string and char literals (strings left as
/// `""`), cut at a column-0 `#[cfg(test)]`.
fn strip(src: &str) -> String {
    let src = match src.find("\n#[cfg(test)]") {
        Some(i) => &src[..i],
        None if src.starts_with("#[cfg(test)]") => "",
        None => src,
    };
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if b[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if (c == 'r' || c == 'b')
            && !out.chars().next_back().is_some_and(is_ident)
            && raw_string_start(&b, i).is_some()
        {
            let (hashes, start) = raw_string_start(&b, i).unwrap();
            let close: Vec<char> = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
            i = start;
            while i < b.len() && !b[i..].starts_with(&close) {
                i += 1;
            }
            i += close.len();
            out.push_str("\"\"");
        } else if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' && (next == Some('\\') || b.get(i + 2) == Some(&'\'')) {
            i += 1;
            while i < b.len() && b[i] != '\'' {
                i += if b[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("''");
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// At a raw string (`r"`, `r#"`, `br"`): its `#` count and where its text
/// starts.
fn raw_string_start(b: &[char], mut i: usize) -> Option<(usize, usize)> {
    if b[i] == 'b' {
        i += 1;
    }
    if b.get(i) != Some(&'r') {
        return None;
    }
    i += 1;
    let mut hashes = 0;
    while b.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    (b.get(i) == Some(&'"')).then_some((hashes, i + 1))
}

/// Byte positions of `word` as a whole word.
fn word_at(code: &str, word: &str) -> Vec<usize> {
    code.match_indices(word)
        .map(|(i, _)| i)
        .filter(|i| {
            // `'static` is a lifetime, not the keyword.
            !code[..*i].chars().next_back().is_some_and(|c| is_ident(c) || c == '\'')
                && !code[i + word.len()..].chars().next().is_some_and(is_ident)
        })
        .collect()
}

/// The end (exclusive) of the bracketed group opening at `open`.
fn group_end(code: &str, open: usize) -> usize {
    let mut depth = 0i32;
    for (i, c) in code[open..].char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + i + 1;
                }
            }
            _ => {}
        }
    }
    code.len()
}

fn ident_after(code: &str, at: usize) -> String {
    code[at..].trim_start().chars().take_while(|c| is_ident(*c)).collect()
}

fn takes_world(params: &str) -> bool {
    ["World", "DeferredWorld", "EntityWorldMut"]
        .iter()
        .any(|w| !word_at(params, w).is_empty())
}

/// Types storing callbacks that take the world: names of the structs,
/// enums and type aliases with a `fn(..)` pointer or `Fn(..)` trait whose
/// parameters name `World`, `DeferredWorld` or `EntityWorldMut`.
fn registries(code: &str) -> BTreeSet<String> {
    // Item keywords: the nearest one before a callback type says where it is.
    let mut items: Vec<(usize, &str)> = Vec::new();
    for kw in [
        "struct", "enum", "type", "fn", "impl", "trait", "const", "static", "let", "mod",
    ] {
        for at in word_at(code, kw) {
            if kw == "fn" && code[at + 2..].trim_start().starts_with('(') {
                continue;
            }
            items.push((at, kw));
        }
    }
    items.sort();
    let mut out = BTreeSet::new();
    for kw in ["fn", "Fn", "FnMut", "FnOnce"] {
        for at in word_at(code, kw) {
            let rest = &code[at + kw.len()..];
            if !rest.trim_start().starts_with('(') {
                continue;
            }
            let open = at + kw.len() + (rest.len() - rest.trim_start().len());
            if !takes_world(&code[open..group_end(code, open)]) {
                continue;
            }
            let Some(&(item, what)) = items.iter().rev().find(|(i, _)| *i < at) else {
                continue;
            };
            if !matches!(what, "struct" | "enum" | "type") || !inside_item(&code[item..at]) {
                continue;
            }
            out.insert(ident_after(code, item + what.len()));
        }
    }
    out
}

/// Whether a position (the end of `head`, which starts at the item's
/// keyword) is still inside the item: its body hasn't closed and it hasn't
/// ended with `;`.
fn inside_item(head: &str) -> bool {
    let mut depth = 0i32;
    let mut opened = false;
    for c in head.chars() {
        match c {
            '(' | '{' | '[' => {
                depth += 1;
                opened = true;
            }
            ')' | '}' | ']' => {
                depth -= 1;
                if depth == 0 && opened && c == '}' {
                    return false;
                }
            }
            ';' if depth == 0 => return false,
            _ => {}
        }
    }
    true
}

/// Functions taking `&mut World`: their names.
fn world_fns(code: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for at in word_at(code, "fn") {
        let name = ident_after(code, at + 2);
        if name.is_empty() {
            continue;
        }
        let after = at + 2 + code[at + 2..].find(&name).unwrap() + name.len();
        let rest = &code[after..];
        // Generics up to the parameter list.
        let Some(open) = rest.find('(') else { continue };
        if rest[..open].contains(['{', ';']) {
            continue;
        }
        let params = &code[after + open..group_end(code, after + open)];
        let flat: String = params.split_whitespace().collect();
        if flat.contains("&mutWorld") || flat.contains("&'_mutWorld") {
            out.insert(name);
        }
    }
    out
}

/// The systems named in per-tick `add_systems` calls: plain names and
/// paths (`flow::update`), in order of appearance.
fn per_tick_systems(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for at in word_at(code, "add_systems") {
        let rest = &code[at + "add_systems".len()..];
        let Some(open) = rest.find('(') else { continue };
        if !rest[..open].trim().is_empty() {
            continue;
        }
        let open = at + "add_systems".len() + open;
        let args = &code[open + 1..group_end(code, open).saturating_sub(1)];
        let (schedule, systems) = split_first_arg(args);
        let schedule = schedule.trim();
        if ONE_OFF_SCHEDULES.iter().any(|s| ident_after(schedule, 0) == *s) {
            continue;
        }
        out.extend(paths(systems));
    }
    out
}

/// The first top-level argument and the rest.
fn split_first_arg(args: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, c) in args.char_indices() {
        match c {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => return (&args[..i], &args[i + 1..]),
            _ => {}
        }
    }
    (args, "")
}

/// Paths in a system expression that aren't calls, method names or
/// arguments of a call: `a`, `m::a`, `a::<T>`.
fn paths(expr: &str) -> Vec<String> {
    let b = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        // A call's arguments (`run_if(cond)`, `after(tick)`): not systems.
        if c == '(' && expr[..i].trim_end().ends_with(|p: char| is_ident(p) || p == '>') {
            i = group_end(expr, i);
            continue;
        }
        if !(c.is_alphabetic() || c == '_') || (i > 0 && is_ident(b[i - 1] as char)) {
            i += 1;
            continue;
        }
        let start = i;
        loop {
            while i < b.len() && is_ident(b[i] as char) {
                i += 1;
            }
            if expr[i..].starts_with("::")
                && expr[i + 2..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
            {
                i += 2;
            } else {
                break;
            }
        }
        let path = &expr[start..i];
        // Generic arguments (`system::<T>`).
        if expr[i..].starts_with("::<") {
            let mut depth = 0;
            for (k, c) in expr[i + 2..].char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    i += 2 + k + 1;
                    break;
                }
            }
        }
        let before = expr[..start].trim_end();
        let after = expr[i..].trim_start();
        let method = before.ends_with('.');
        let call = after.starts_with('(') || after.starts_with('!') || after.starts_with('{');
        if !method && !call {
            out.push(path.to_string());
        }
    }
    out
}

/// A file's module path in its crate (`src/mode/flow.rs` -> `mode::flow`).
fn module_path(rel: &str) -> String {
    let mut parts: Vec<&str> = rel.trim_end_matches(".rs").split('/').collect();
    if matches!(parts.last(), Some(&"mod" | &"lib" | &"main")) {
        parts.pop();
    }
    parts.join("::")
}

fn skipped(rel: &str) -> bool {
    rel.ends_with("tests.rs") || rel.split('/').any(|p| p == "tests" || p == "ui" || p == "ui.rs")
}

/// One crate's scanned files: (path from the repository root, path from
/// `src/`, stripped code).
fn crate_files(krate: &str) -> Vec<(String, String, String)> {
    let src = root().join(krate).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let rel = file.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        if skipped(&rel) {
            continue;
        }
        let code = strip(&fs::read_to_string(&file).unwrap());
        out.push((format!("{krate}/src/{rel}"), rel, code));
    }
    out
}

/// A crate's per-tick exclusive systems: (file from the repository root,
/// function).
fn exclusive_systems(files: &[(String, String, String)]) -> BTreeSet<(String, String)> {
    let fns: Vec<(&str, String, BTreeSet<String>)> = files
        .iter()
        .map(|(path, rel, code)| (path.as_str(), module_path(rel), world_fns(code)))
        .collect();
    let mut out = BTreeSet::new();
    for ((path, _, code), (_, _, own)) in files.iter().zip(&fns) {
        for system in per_tick_systems(code) {
            let mut segs: Vec<&str> = system.split("::").collect();
            let name = segs.pop().unwrap();
            while matches!(segs.first(), Some(&"crate" | &"self" | &"super")) {
                segs.remove(0);
            }
            if segs.is_empty() {
                if own.contains(name) {
                    out.insert((path.clone(), name.to_string()));
                }
                continue;
            }
            let module = segs.join("::");
            for (other, path_of, theirs) in &fns {
                if theirs.contains(name) && (*path_of == module || path_of.ends_with(&format!("::{module}"))) {
                    out.insert((other.to_string(), name.to_string()));
                }
            }
        }
    }
    out
}

/// Hook kinds keyed by game (`chimera_core::gameplay::GameHooks`, plan
/// step 2): registered with `game_hook`/`any_game_hook`, never a resource
/// of their own that the last plugin added would replace.
const KEYED_HOOKS: &[&str] = &["JoinRules", "Holds", "BotHooks", "NewGame", "LogSubjects"];

#[test]
fn keyed_hooks_are_not_single_slot_resources() {
    let mut errors = Vec::new();
    for krate in crates() {
        for (path, _, code) in crate_files(&krate) {
            let squeezed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
            for hook in KEYED_HOOKS {
                let as_resource = [
                    "insert_resource(",
                    "init_resource::<",
                    "get_resource::<",
                    "resource::<",
                    "resource_mut::<",
                    "Res<",
                ]
                .iter()
                .any(|call| {
                    squeezed.match_indices(call).any(|(at, _)| {
                        let rest = &squeezed[at + call.len()..];
                        // The type's path (`crate::gameplay::NewGame`): its last segment.
                        let path = rest
                            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                            .next()
                            .unwrap_or("");
                        path.rsplit("::").next() == Some(hook)
                    })
                });
                if as_resource {
                    errors.push(format!(
                        "{path}: `{hook}` used as a resource. It is keyed by game: register a \
                         game's with `app.game_hook(<game id>, {hook} {{ .. }})` \
                         (`chimera_core::gameplay::GameplayAppExt`) and read the gameplay's with \
                         `gameplay::active_hook` (the network's `{hook}::active`), so another \
                         game's plugin doesn't replace it. See docs/DESIGN.md, \"ECS style\"."
                    ));
                }
            }
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

#[test]
fn callback_registries_are_allowed() {
    let mut found = BTreeSet::new();
    for krate in crates() {
        for (path, _, code) in crate_files(&krate) {
            for ty in registries(&code) {
                found.insert((path.clone(), ty));
            }
        }
    }
    let mut errors = Vec::new();
    for (file, ty) in &found {
        if !ALLOWED_REGISTRIES.iter().any(|(f, t, _)| f == file && t == ty) {
            errors.push(format!(
                "{file}: `{ty}` stores whole-world callbacks (`fn(&mut World, ..)`), which hide what \
                 they read and write and run outside the schedule's order. Prefer, in order: a \
                 component that games observe (`On<Insert<C>>`/`On<Discard<C>>`, as \
                 `ladder::OnRung`); an event triggered at the same point (`world.trigger`, observers \
                 run at once); a message read by a normal system; a system gated by a run condition \
                 (`gameplay_is`, `hud_is`). A decision the runtime asks (`fn(&World, ..) -> answer`) \
                 is fine. If the job really needs the whole world (map loading, spawning, network \
                 apply, codecs, console, tools), add (\"{file}\", \"{ty}\", \"<why>\") to \
                 ALLOWED_REGISTRIES in checks/tests/it/ecs.rs. See docs/DESIGN.md, \"ECS style\"."
            ));
        }
    }
    for (file, ty, _) in ALLOWED_REGISTRIES {
        if !found.contains(&(file.to_string(), ty.to_string())) {
            errors.push(format!(
                "{file}: `{ty}` is on ALLOWED_REGISTRIES in checks/tests/it/ecs.rs but stores no \
                 whole-world callback there any more (moved, renamed or converted): remove or fix \
                 its row."
            ));
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

#[test]
fn per_tick_exclusive_systems_ratchet() {
    let mut errors = Vec::new();
    let mut used_files = BTreeSet::new();
    let crates = crates();
    for (krate, _) in EXCLUSIVE_BASELINE {
        if !crates.iter().any(|c| c == krate) {
            errors.push(format!(
                "{krate}: in EXCLUSIVE_BASELINE in checks/tests/it/ecs.rs but no scanned crate: \
                 remove its row."
            ));
        }
    }
    let mut by_crate: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for krate in &crates {
        let systems = exclusive_systems(&crate_files(krate));
        let mut counted = Vec::new();
        for (file, name) in systems {
            if ALLOWED_EXCLUSIVE_FILES.iter().any(|(f, _)| *f == file) {
                used_files.insert(file);
                continue;
            }
            counted.push(format!("{file}: {name}"));
        }
        by_crate.insert(krate.clone(), counted);
    }
    for (krate, counted) in &by_crate {
        let stored = EXCLUSIVE_BASELINE
            .iter()
            .find(|(c, _)| c == krate)
            .map_or(0, |(_, n)| *n);
        let now = counted.len();
        if now > stored {
            errors.push(format!(
                "{krate}: {now} exclusive systems (taking `&mut World`) run every tick or frame, \
                 {stored} allowed (+{}). Counted: {}. Write a new one as a system with declared \
                 parameters (`Query`, `Res`, `MessageReader`, `Commands`), gated by a run condition \
                 (`gameplay_is`, `hud_is`; never a new set); a reaction to a moment as an observer. \
                 If its job is the world (network apply or capture, map loading, the console, a \
                 menu), put it in a file on ALLOWED_EXCLUSIVE_FILES in checks/tests/it/ecs.rs with \
                 the reason. See docs/DESIGN.md, \"ECS style\".",
                now - stored,
                counted.join(", ")
            ));
        } else if now < stored {
            errors.push(format!(
                "{krate}: {now} exclusive systems run every tick or frame, but EXCLUSIVE_BASELINE in \
                 checks/tests/it/ecs.rs allows {stored}: lower the stored number to {now} (the \
                 counts only fall; remove the row at 0)."
            ));
        }
    }
    for (file, _) in ALLOWED_EXCLUSIVE_FILES {
        if !used_files.contains(*file) {
            errors.push(format!(
                "{file}: on ALLOWED_EXCLUSIVE_FILES in checks/tests/it/ecs.rs but registers no \
                 per-tick exclusive system: remove its row."
            ));
        }
    }
    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
}

/// The scanners find what they should (so quiet checks mean nothing to
/// report).
#[test]
fn ecs_scanners_work() {
    let code = strip(
        "pub struct Hooks { pub update: fn(&mut World), pub check: fn(u8) -> bool }\n\
         pub struct Pure { pub f: fn(u8) }\n\
         pub type Launch = fn(&mut World, Entity);\n\
         pub struct One(pub fn(&World) -> bool);\n\
         pub struct Dyn { run: Arc<dyn Fn(&mut World, &[String]) + Send + Sync> }\n\
         pub struct Ent { name: &'static str, insert: fn(&mut EntityWorldMut) }\n\
         fn register(f: fn(&mut World)) {}\n\
         const TABLE: &[fn(&mut World)] = &[];\n\
         // pub struct Commented { f: fn(&mut World) }\n\
         fn s() -> &'static str { \"struct InString { f: fn(&mut World) }\" }\n",
    );
    let found: Vec<String> = registries(&code).into_iter().collect();
    assert_eq!(found, ["Dyn", "Ent", "Hooks", "Launch", "One"]);

    let code = strip(
        "fn tick(world: &mut World) {}\n\
         fn once(world: &mut World) {}\n\
         fn reader(q: Query<&A>) {}\n\
         fn other<T: X>(world: &mut World, e: Entity) {}\n\
         fn plugin(app: &mut App) {\n\
             app.add_systems(Startup, once)\n\
                .add_systems(OnEnter(State::A), other::<u8>)\n\
                .add_systems(FixedUpdate, (tick, reader, flow::update).chain().run_if(on(x)))\n\
                .add_systems(Update, other::<u8>.after(tick));\n\
         }\n\
         #[cfg(test)]\n\
         fn t(app: &mut App) { app.add_systems(Update, once); }\n",
    );
    let fns: Vec<String> = world_fns(&code).into_iter().collect();
    assert_eq!(fns, ["once", "other", "tick"]);
    assert_eq!(per_tick_systems(&code), ["tick", "reader", "flow::update", "other"]);
    assert_eq!(module_path("mode/flow.rs"), "mode::flow");
    assert_eq!(module_path("net/mod.rs"), "net");
    assert_eq!(
        strip("let c = '\"'; let s = r#\"a \" b\"#; x"),
        "let c = ''; let s = \"\"; x"
    );
}
