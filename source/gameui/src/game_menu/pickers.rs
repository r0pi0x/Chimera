//! Options > Multiplayer's player model and spray pickers, Import Spray,
//! and Create Server's Server Type (specs/hl2dm/game_ui.md 2.1-2.3, 4).
//!
//! A picker is a combo whose entries each pair a text with a console line
//! (GameUI's command combo): choosing one only changes the preview picture
//! and enables Apply; OK or Apply runs the chosen entry's line (`cl_playermodel
//! models/<path>.mdl`, `cl_logofile materials/vgui/logos/<stem>.vtf`),
//! Cancel drops it. When the cvar matches no entry (`none`, a fresh
//! config) the first entry (the spray: `spray_canned`) shows and nothing
//! runs until the player changes the selection. Import Spray is an
//! in-game dialog with a file path (Chimera has no native file dialog
//! dependency), its errors in a "Spray Import Error" box.

use super::*;

/// Which picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Model,
    Spray,
}

impl PickerKind {
    /// The layout control (`fieldName`) of the combo.
    pub fn field(self) -> &'static str {
        match self {
            PickerKind::Model => "Player model",
            PickerKind::Spray => "SpraypaintList",
        }
    }

    /// The layout control that shows the selection's picture.
    pub fn preview(self) -> &'static str {
        match self {
            PickerKind::Model => "ModelImage",
            PickerKind::Spray => "LogoImage",
        }
    }

    /// The cvar it sets.
    pub fn cvar(self) -> &'static str {
        match self {
            PickerKind::Model => "cl_playermodel",
            PickerKind::Spray => "cl_logofile",
        }
    }
}

/// An entry of a picker.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickerEntry {
    /// Its text in the combo (the file stem).
    pub text: String,
    /// The cvar's value it stands for.
    pub value: String,
    /// Its picture's name in `MenuUi::pictures`.
    pub picture: String,
}

/// A picker's entries and selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picker {
    pub entries: Vec<PickerEntry>,
    /// The entry shown selected.
    pub selected: usize,
    /// The entry shown when it opened or was last applied; a different
    /// `selected` is a change OK or Apply runs.
    pub base: usize,
}

impl Picker {
    /// The player chose another entry than the one applied.
    pub fn pending(&self) -> bool {
        !self.entries.is_empty() && self.selected != self.base
    }

    /// The selected entry.
    pub fn current(&self) -> Option<&PickerEntry> {
        self.entries.get(self.selected)
    }
}

/// Both pickers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pickers {
    pub model: Picker,
    pub spray: Picker,
}

impl Pickers {
    pub fn get(&self, kind: PickerKind) -> &Picker {
        match kind {
            PickerKind::Model => &self.model,
            PickerKind::Spray => &self.spray,
        }
    }

    fn get_mut(&mut self, kind: PickerKind) -> &mut Picker {
        match kind {
            PickerKind::Model => &mut self.model,
            PickerKind::Spray => &mut self.spray,
        }
    }
}

/// The note under Import Spray's path box: what it takes.
const IMPORT_NOTE: &str = "Type the path of a .tga, .jpg or .vtf picture. A .tga (24-bit RGB or 32-bit RGBA) or .jpg \
                           needs a width and a height of 256, 128, 64, 32, 16, 8, 4, 2 or 1.";

/// The case-insensitive order of file names.
fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// The same value, however it is spelled.
fn same(a: &str, b: &str) -> bool {
    a.trim().replace('\\', "/").eq_ignore_ascii_case(b.trim().replace('\\', "/").as_str())
}

impl GameMenu {
    /// The entries the player model picker lists: one per picture the
    /// install has, sorted by path, with the file stem as text.
    fn model_entries(&self) -> Vec<PickerEntry> {
        let Some(ui) = self.ui.0.as_ref() else { return Vec::new() };
        sorted(ui.player_models.clone())
            .into_iter()
            .map(|p| PickerEntry {
                text: p.rsplit('/').next().unwrap_or(&p).to_string(),
                value: format!("models/{p}.mdl"),
                picture: format!("playermodels/{p}"),
            })
            .collect()
    }

    /// The entries the spray picker lists: the install's logos and the
    /// imported ones, sorted by stem.
    fn spray_entries(&self) -> Vec<PickerEntry> {
        let installed = self.ui.0.as_ref().map(|u| u.sprays.clone()).unwrap_or_default();
        sorted(installed.into_iter().chain(self.imported.iter().cloned()).collect())
            .into_iter()
            .map(|s| PickerEntry {
                value: format!("materials/vgui/logos/{s}.vtf"),
                picture: format!("logos/{s}"),
                text: s,
            })
            .collect()
    }

    /// Set the pickers for opening: each shows the entry the cvar names,
    /// else its first (the spray: `spray_canned`), as displayed only.
    pub(super) fn open_pickers(&mut self, get: &dyn Fn(&str) -> Option<String>) {
        self.imported = self.logos_dir.as_deref().map(source_engine::hud::spray_import::imported).unwrap_or_default();
        let mut pickers = Pickers::default();
        if self.game.player_models() {
            pickers.model.entries = self.model_entries();
        }
        if self.game.sprays() {
            pickers.spray.entries = self.spray_entries();
        }
        for kind in [PickerKind::Model, PickerKind::Spray] {
            let p = pickers.get_mut(kind);
            let now = get(kind.cvar()).unwrap_or_default();
            let default = match kind {
                PickerKind::Spray => p.entries.iter().position(|e| e.text.eq_ignore_ascii_case("spray_canned")).unwrap_or(0),
                PickerKind::Model => 0,
            };
            p.selected = p.entries.iter().position(|e| same(&e.value, &now)).unwrap_or(default);
            p.base = p.selected;
        }
        self.pickers = pickers;
    }

    /// Whether a picker has a change OK or Apply would run.
    pub(super) fn pickers_pending(&self) -> bool {
        self.pickers.model.pending() || self.pickers.spray.pending()
    }

    /// OK or Apply: run the lines of the entries chosen.
    pub(super) fn apply_pickers(&mut self, out: &mut Outcome) {
        for kind in [PickerKind::Model, PickerKind::Spray] {
            let p = self.pickers.get_mut(kind);
            if p.pending()
                && let Some(e) = p.current()
            {
                out.lines.push(format!("{} {}", kind.cvar(), e.value));
                p.base = p.selected;
            }
        }
    }

    /// Cancel: the choices dropped.
    pub(super) fn revert_pickers(&mut self) {
        for kind in [PickerKind::Model, PickerKind::Spray] {
            let p = self.pickers.get_mut(kind);
            p.selected = p.base;
        }
    }

    /// Choose entry `k` of a picker (the preview follows at once).
    pub(super) fn pick_entry(&mut self, kind: PickerKind, k: usize) {
        let p = self.pickers.get_mut(kind);
        if k < p.entries.len() {
            p.selected = k;
        }
    }

    /// A picker's row: its combo (None where the game or the layout has
    /// none, or the install has no pictures).
    pub(super) fn picker_row(&self, kind: PickerKind) -> Option<Row> {
        let p = self.pickers.get(kind);
        let shown = match kind {
            PickerKind::Model => self.game.player_models(),
            PickerKind::Spray => self.game.sprays(),
        };
        if !shown || p.entries.is_empty() {
            return None;
        }
        if self.layout().is_some_and(|l| l.get(kind.field()).is_none()) {
            return None;
        }
        let entries: Vec<String> = p.entries.iter().map(|e| e.text.clone()).collect();
        let (token, ours) = match kind {
            PickerKind::Model => ("#GameUI_PlayerModel", "Player model"),
            PickerKind::Spray => ("#GameUI_SpraypaintImage", "Spraypaint image"),
        };
        Some(Row::Control {
            label: self.text(token, ours),
            field: Field::Picker(kind),
            control: Control::Combo {
                text: entries.get(p.selected).cloned().unwrap_or_default(),
                selected: Some(p.selected),
                entries,
            },
        })
    }

    /// The picture name the preview of `kind` shows.
    pub fn preview_picture(&self, kind: PickerKind) -> Option<&str> {
        self.pickers.get(kind).current().map(|e| e.picture.as_str())
    }

    /// Whether the Multiplayer page has the Import Spray button.
    pub(super) fn has_import(&self) -> bool {
        self.game.sprays() && self.layout().is_none_or(|l| l.get("ImportSprayImage").is_some())
    }

    /// Import Spray's dialog.
    pub(super) fn import_rows(&self) -> Vec<Row> {
        vec![
            Row::Info(self.text("#GameUI_ImportSprayImage", "Import Spray Image")),
            Row::Info(IMPORT_NOTE.into()),
            Row::Control {
                label: "File".into(),
                field: Field::ImportPath,
                control: Control::Text {
                    text: self.import_path.clone(),
                    numeric: false,
                    max: 260,
                },
            },
            Self::button_row("Import", Action::ImportDo),
            self.cancel_row(),
        ]
    }

    /// The error box's rows.
    pub(super) fn import_error_rows(&self) -> Vec<Row> {
        vec![Row::Info(self.import_error.clone().unwrap_or_default()), self.ok_row()]
    }

    /// Import Spray's OK: the file converted into the user's spray folder,
    /// the list rebuilt, the new spray selected; or the error box.
    pub(super) fn import_spray(&mut self) {
        use source_engine::hud::spray_import as si;
        let typed = self.import_path.trim().trim_matches('"').to_string();
        let result = match self.logos_dir.clone() {
            Some(dir) => si::import(std::path::Path::new(&typed), &dir),
            None => Err(si::ImportError::CannotWrite),
        };
        match result {
            Ok(stem) => {
                // The list rebuilt; what was applied stays so, the new
                // spray selected (taking effect on OK or Apply).
                let applied = self.pickers.spray.current_base().map(|e| e.value.clone());
                self.imported = self.logos_dir.as_deref().map(si::imported).unwrap_or_default();
                let entries = self.spray_entries();
                let value = format!("materials/vgui/logos/{stem}.vtf");
                let selected = entries.iter().position(|e| same(&e.value, &value)).unwrap_or(0);
                let base = applied.and_then(|a| entries.iter().position(|e| same(&e.value, &a))).unwrap_or(selected);
                self.pickers.spray = Picker { entries, selected, base };
                self.import_path.clear();
                self.back();
            }
            Err(e) => {
                self.import_error = Some(e.text().to_string());
                self.go_to(Page::SprayError);
            }
        }
    }
}

impl Picker {
    /// The entry applied (shown when it opened or was last applied).
    fn current_base(&self) -> Option<&PickerEntry> {
        self.entries.get(self.base)
    }
}

/// Create Server's Server Type (specs/hl2dm/game_ui.md 4): the token and
/// our text of each value, in the order shown, and the console lines
/// Start runs for it.
pub const SERVER_TYPES: [(&str, &str, &[&str]); 4] = [
    (
        "#Server_Visibility_Local",
        "Local Network (LAN)",
        &["sv_lan 1", "sv_allow_server_adverisement_to_master_server 0", "sv_friends_only 0"],
    ),
    (
        "#Server_Visibility_Unlisted",
        "Public (Unlisted)",
        &["sv_lan 0", "sv_allow_server_adverisement_to_master_server 0", "sv_friends_only 0"],
    ),
    (
        "#Server_Visibility_Global",
        "Public (Listed)",
        &["sv_lan 0", "setmaster enable", "sv_allow_server_adverisement_to_master_server 1", "sv_friends_only 0"],
    ),
    (
        "#Server_Visibility_FriendsOnly",
        "Friends Only",
        &["sv_lan 0", "sv_allow_server_adverisement_to_master_server 0", "sv_friends_only 1"],
    ),
];

/// The archived cvar remembering the last Server Type (an index into
/// `SERVER_TYPES`; 0, LAN, on a fresh config).
pub const SERVER_TYPE_CVAR: &str = "chimera_server_type";

impl GameMenu {
    /// The Server Type combo's row.
    pub(super) fn visibility_row(&self) -> Row {
        let entries: Vec<String> = SERVER_TYPES.iter().map(|(t, o, _)| self.text(t, o)).collect();
        let k = self.new_game.visibility.min(SERVER_TYPES.len() - 1);
        Row::Control {
            label: self.text("#Server_Visibility", "Server Type"),
            field: Field::Visibility,
            control: Control::Combo {
                text: entries[k].clone(),
                selected: Some(k),
                entries,
            },
        }
    }

    /// The lines Start runs for the Server Type.
    pub(super) fn visibility_lines(&self) -> Vec<String> {
        let k = self.new_game.visibility.min(SERVER_TYPES.len() - 1);
        SERVER_TYPES[k].2.iter().map(|l| l.to_string()).chain([format!("{SERVER_TYPE_CVAR} {k}")]).collect()
    }
}
