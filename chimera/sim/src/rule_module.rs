//! Rule modules: a game's round flow, scoring and win conditions,
//! registered by the game under a namespaced name (`cs_source:bomb`)
//! and picked by the map and the cvars, as a minigame definition will pick
//! one by name (docs/plans/active/engine-split.md, "Minigames").
//!
//! Data first: a module is a description (its name, what it plays, the
//! cvars that set it up) plus when it applies; its behaviour is the
//! systems its game adds. The first registered module that applies is the
//! one playing (`pick`; `chimera_rules` lists them). So far the modules describe the code in
//! `rules` and `objectives`, which runs as before; moving that code under
//! its game is the next step (engine-split.md).

use bevy::prelude::*;

use crate::console::ConsoleAppExt;

/// A rule module as its game registers it.
#[derive(Clone, Debug)]
pub struct RuleModule {
    /// `game:name`, e.g. `cs_source:bomb`.
    pub name: &'static str,
    /// What it plays, one line.
    pub description: &'static str,
    /// The cvars that set it up (its settings, set as data: configs,
    /// `--cvar`, a minigame definition).
    pub settings: &'static [&'static str],
    /// Whether it plays now (the map's objectives, the cvars).
    pub applies: fn(&World) -> bool,
}

/// The registered rule modules, in the order they are tried.
#[derive(Resource, Clone, Debug, Default)]
pub struct RuleModules(pub Vec<RuleModule>);

impl RuleModules {
    /// Add `module` (a game's plugin, as it builds); one of the same name
    /// is replaced in place.
    pub fn register(app: &mut App, module: RuleModule) {
        let mut modules = app.world_mut().get_resource_or_init::<RuleModules>();
        match modules.0.iter_mut().find(|m| m.name == module.name) {
            Some(m) => *m = module,
            None => modules.0.push(module),
        }
    }

    pub fn get(&self, name: &str) -> Option<&RuleModule> {
        self.0.iter().find(|m| m.name == name)
    }
}

/// A module's settings document (TOML; `name` for errors) read as its
/// settings type `T`: a game ships its modes' numbers as data and reads
/// them here (`T` with `#[serde(deny_unknown_fields)]` makes unknown keys
/// load errors). The cvars set them after.
pub fn parse_settings<T: serde::de::DeserializeOwned>(name: &str, text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| format!("{name}: {e}"))
}

/// The first registered module that applies.
pub fn pick(world: &World) -> Option<&'static str> {
    let modules = world.get_resource::<RuleModules>()?;
    modules.0.iter().find(|m| (m.applies)(world)).map(|m| m.name)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RuleModules>().console_command(
        "chimera:rules",
        "List the rule modules the games registered (name, what it plays, its cvars), the one playing marked *.",
        |w, _| {
            let active = pick(w);
            let modules = w.get_resource::<RuleModules>().map(|m| m.0.clone()).unwrap_or_default();
            if modules.is_empty() {
                return Ok(Some("no rule modules registered".into()));
            }
            let lines: Vec<String> = modules
                .iter()
                .map(|m| {
                    let mark = if Some(m.name) == active { "*" } else { " " };
                    format!("{mark} {}: {} ({})", m.name, m.description, m.settings.join(", "))
                })
                .collect();
            Ok(Some(lines.join("\n")))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_module_that_applies_plays() {
        let mut app = App::new();
        let module = |name, applies| RuleModule {
            name,
            description: "",
            settings: &[],
            applies,
        };
        RuleModules::register(&mut app, module("a:never", |_| false));
        RuleModules::register(&mut app, module("a:always", |_| true));
        RuleModules::register(&mut app, module("a:also", |_| true));
        assert_eq!(pick(app.world()), Some("a:always"));
        // Registered again: replaced where it was.
        RuleModules::register(&mut app, module("a:never", |_| true));
        assert_eq!(pick(app.world()), Some("a:never"));
        assert_eq!(app.world().resource::<RuleModules>().0.len(), 3);
    }
}
