//! Where each TF2 gun's numbers come from (docs/plans/active/tf2.md,
//! section 3, decision 3): the game's weapon script when one is served
//! (`scripts::read`: the asset store's decrypted record, or the install
//! decrypted with the user's key), else the spec's public numbers that
//! data/weapons/*.toml commit (the fallback). `note_sources`, at each load
//! of TF2's guns (a map's kit, another game's map dressed with them), asks
//! for each gun's script and logs, loudly and once per gun, which source
//! its numbers came from.
//!
//! The seam: `numbers_from_script` is where the scripts' field mapping
//! goes (which KeyValues field is the damage, the interval, the clip,
//! the spread...). No spec names those fields yet (no one with the key
//! has documented them), so it maps nothing: every gun says "spec
//! fallback", with what was found.
//!
//! Known shortcuts:
//! - The scripts' field mapping (`numbers_from_script`) is empty: a served
//!   script is noted but changes no number until a spec written with the
//!   key names its fields.

use super::super::scripts::{self, Script, ScriptSource};
use crate::mount::Mount;

/// Numbers a script gives a gun (each None: the data's stays).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptNumbers {
    pub damage: Option<f32>,
    pub pellets: Option<u32>,
    pub interval: Option<f32>,
    pub clip: Option<u32>,
    pub spread: Option<f32>,
}

impl ScriptNumbers {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The field mapping: a script's numbers. Empty until the fields are
/// specced (module doc).
pub fn numbers_from_script(_script: &Script) -> ScriptNumbers {
    ScriptNumbers::default()
}

/// Where a gun's numbers came from, as the log says it.
pub fn source_of(script: &Script, numbers: &ScriptNumbers) -> String {
    match (script.source, numbers.is_empty()) {
        (ScriptSource::Missing, _) => format!(
            "spec fallback (specs/tf2/weapons.md; no script: {})",
            script.note.as_deref().unwrap_or("not found")
        ),
        (source, true) => format!(
            "spec fallback (script {} read from {}, but its fields aren't mapped yet)",
            script.name,
            source.describe()
        ),
        (source, false) => format!("script {} from {}", script.name, source.describe()),
    }
}

/// Each gun's script asked for through `mount` and its numbers' source
/// logged, once per gun (a warning while it is the spec's).
pub fn note_sources(mount: &Mount) {
    let key = scripts::key_or_warn();
    for (gun, script_name) in super::GUNS.iter().map(|g| {
        (*g, super::hook_params::<super::damage::GunParams>(g, super::GUN_HOOK).script)
    }) {
        let script = scripts::read(mount, &script_name, key.as_ref());
        let numbers = numbers_from_script(&script);
        let source = source_of(&script, &numbers);
        if numbers.is_empty() {
            bevy::log::warn!("tf2: {gun}'s numbers: {source}");
        } else {
            bevy::log::info!("tf2: {gun}'s numbers: {source}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_formats::keyvalues::Kv;

    #[test]
    fn without_a_mapping_every_gun_says_spec_fallback() {
        let missing = Script { name: "tf_weapon_minigun".into(), source: ScriptSource::Missing, root: Kv::Block(Vec::new()), note: Some("no key".into()) };
        assert!(source_of(&missing, &numbers_from_script(&missing)).starts_with("spec fallback"));
        let plain = Script { source: ScriptSource::Plain, note: None, ..missing };
        let s = source_of(&plain, &numbers_from_script(&plain));
        assert!(s.starts_with("spec fallback") && s.contains("aren't mapped"), "{s}");
    }
}
