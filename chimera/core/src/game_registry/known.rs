//! Every game registered in this process, by any app
//! (`game_registry::Games::register`): what names a game where no world is
//! at hand (pickers, headings, cards), so no table of game names is kept
//! anywhere else and a new game needs no edit there. Only the games'
//! static descriptions (`GameInfo`), the same for every app of the
//! process; never read by the simulation.

use super::GameInfo;

/// Note `game` as registered (once per id).
pub(crate) fn note(game: &'static GameInfo) {
    let mut known = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if !known.iter().any(|g| g.id == game.id) {
        known.push(game);
    }
}

/// Every game registered in this process, by any app (`Games::register`).
static KNOWN: std::sync::Mutex<Vec<&'static GameInfo>> = std::sync::Mutex::new(Vec::new());

/// The game with id `id` registered by any app of this process: what
/// names a game where no world is at hand (pickers, headings), so no
/// table of game names is kept anywhere else.
pub fn known(id: &str) -> Option<&'static GameInfo> {
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).iter().copied().find(|g| g.id.eq_ignore_ascii_case(id))
}

/// Every game registered in this process, in registration order.
pub fn all_known() -> Vec<&'static GameInfo> {
    KNOWN.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Game `id`'s full name (`GameInfo::name`); `Other` for no game, the id
/// with spaces for an unknown one.
pub fn full_name(id: &str) -> String {
    match known(id) {
        Some(g) => g.name.to_string(),
        None if id.is_empty() => "Other".into(),
        None => id.replace('_', " "),
    }
}

/// Game `id`'s short name (`GameInfo::short_name`); the id upper-cased for
/// an unknown one.
pub fn short_name(id: &str) -> String {
    known(id).map_or_else(|| id.to_ascii_uppercase(), |g| g.short_name.to_string())
}
