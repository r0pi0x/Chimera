//! One component library (docs/backlog.md, "One component library, used
//! everywhere"): the kit's controls (`chimera_ui::controls`: `text_field`,
//! `search_field`, `text_area`, `chat_field`, ...) are the only text
//! inputs of the UGG look. A screen that builds its own (a caret laid out
//! in the line pushes the text about as it moves; every copy looks and
//! acts a little differently) fails here. Building a control by hand is
//! fine when it adds real value the kit lacks; it is not fine for what the
//! kit already matches, and a missing feature is first an option on the
//! kit's component (as optional props on a React or Compose component),
//! not a copy.
//!
//! - `text_inputs_are_the_kits`: outside `chimera/ui`, no file names the
//!   kit's caret and selection markers (`TextCaret`, `TextSelection`:
//!   only the kit's builders make them), defines a function that builds a
//!   caret (`fn ...caret...` taking `Commands`), draws a VGUI text entry
//!   (`text_entry(`) or an egui `TextEdit`, unless the file is on
//!   `ALLOWED` with its reason. An entry that matches nothing any more
//!   fails too, so the list stays true.
//!
//! The scan is the text scan of `ecs.rs` (comments and string literals
//! stripped, cut at a column-0 `#[cfg(test)]`); `tests/` folders are
//! skipped.

use std::{collections::BTreeSet, fs, path::Path};

use super::{
    architecture::{crate_groups, root, rust_files},
    ecs::strip,
};

/// Files that may build their own text input, and why. Everything else
/// uses the kit.
const ALLOWED: &[(&str, &str)] = &[
    (
        "chimera/client/src/client/widgets/look.rs",
        "the VGUI-look widgets (`text_entry`, `Caret`): the games' own GameUI as their installs draw it, not the UGG kit's look",
    ),
    ("chimera/client/src/client/debug_ui.rs", "the developer debug UI is egui (`F`-key inspector), not a player's screen"),
    ("source/gameui/src/first_run.rs", "Source's GameUI panels as CS:S draws them (VGUI widgets)"),
    ("source/gameui/src/game_menu/draw.rs", "Source's GameUI panels as CS:S draws them (VGUI widgets)"),
    ("source/gameui/src/server_browser.rs", "Source's GameUI server browser as CS:S draws it (VGUI widgets)"),
    ("apps/party_kit/src/main_menu.rs", "the party apps' first main menu, drawn in CS:S's GameUI look with the VGUI widgets (the UGG menus are `signal_menu`)"),
];

/// What names a hand-built text input in stripped source: the reason for
/// each hit.
fn hits(code: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    for word in ["TextCaret", "TextSelection"] {
        if has_word(code, word) {
            out.push("names the kit's caret/selection marker (`TextCaret`/`TextSelection`)");
            break;
        }
    }
    if code.contains("text_entry(") {
        out.push("draws a VGUI `text_entry`");
    }
    if code.contains("TextEdit::") {
        out.push("draws an egui `TextEdit`");
    }
    // `fn <...caret...>(` whose parameters take `Commands`.
    let mut rest = code;
    while let Some(i) = rest.find("fn ") {
        let after = &rest[i + 3..];
        let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let sig = &after[name.len()..after.len().min(name.len() + 300)];
        if name.contains("caret") && sig.split(')').next().is_some_and(|p| p.contains("Commands")) {
            out.push("defines a function that builds a caret");
            break;
        }
        rest = after;
    }
    out
}

fn has_word(code: &str, word: &str) -> bool {
    let b = code.as_bytes();
    code.match_indices(word).any(|(i, _)| {
        let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        !(i > 0 && ident(b[i - 1])) && !b.get(i + word.len()).is_some_and(|c| ident(*c))
    })
}

/// Every `src` file of every workspace crate outside the kit, as
/// (repo-relative path, stripped text).
fn sources() -> Vec<(String, String)> {
    let root = root();
    let mut files = Vec::new();
    for group in crate_groups() {
        let Ok(entries) = fs::read_dir(root.join(&group)) else { continue };
        for e in entries.flatten() {
            let src = e.path().join("src");
            if src.is_dir() {
                rust_files(&src, &mut files);
            }
        }
    }
    let mut out = Vec::new();
    for f in files {
        let rel = rel_of(&root, &f);
        if rel.starts_with("chimera/ui/") || rel.contains("/tests/") {
            continue;
        }
        let text = fs::read_to_string(&f).unwrap_or_default();
        out.push((rel, strip(&text)));
    }
    out.sort();
    out
}

fn rel_of(root: &Path, f: &Path) -> String {
    f.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/")
}

#[test]
fn text_inputs_are_the_kits() {
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(f, _)| *f).collect();
    let mut used = BTreeSet::new();
    let mut bad = Vec::new();
    for (rel, code) in sources() {
        let h = hits(&code);
        if h.is_empty() {
            continue;
        }
        if allowed.contains(rel.as_str()) {
            used.insert(rel);
        } else {
            bad.push(format!("  {rel}: {}", h.join("; ")));
        }
    }
    assert!(
        bad.is_empty(),
        "hand-built text inputs outside chimera_ui (docs/backlog.md, \"One component library\"):\n{}\n\n\
         The kit's `Ui::text_field` (one line), `Ui::search_field` (glass and clear key), `Ui::text_area` (several \
         lines) and `Ui::chat_field` (`chimera_ui::controls`) draw one node a character with an overlay caret, so \
         typing moves nothing, and `TextHits`/`TextPress` place a click. Do this, in order:\n\
         1. Use the kit's control as it is.\n\
         2. If your screen needs something extra (an icon, a key hint, a clear key, a different size, a prefix), \
         check whether it can be an optional setting on the kit's component, as an optional prop on a React or \
         Jetpack Compose component, and add that option to `chimera/ui/src/controls.rs` (with its state in the \
         `ui_scene fields` gallery) instead of copying the control.\n\
         3. Only when it adds material value the kit cannot take as an option (another game's own look, a developer \
         tool), put the file on `ALLOWED` in checks/tests/it/ui_kit.rs with a written reason.",
        bad.join("\n")
    );
    let stale: Vec<&&str> = allowed.iter().filter(|f| !used.contains(**f)).collect();
    assert!(
        stale.is_empty(),
        "ALLOWED in checks/tests/it/ui_kit.rs lists files that no longer build a text input (or are gone): {stale:?}. \
         Remove the entries."
    );
}

#[test]
fn the_text_input_scan_sees_what_it_should() {
    assert!(hits("let c = TextCaret;").len() == 1);
    assert!(hits("fn draw_caret(c: &mut Commands, p: Entity) {}").len() == 1);
    assert!(hits("fn caret_at(text: &str) -> usize { 0 }").is_empty(), "a function that only computes a place is fine");
    assert!(hits("let x = MyTextCaretLike;").is_empty());
    assert!(hits("widgets::text_entry(c, f);").len() == 1);
    assert!(hits("ui.text_field(c, p, v, \"\", None, false, 1.0, s);").is_empty());
}
