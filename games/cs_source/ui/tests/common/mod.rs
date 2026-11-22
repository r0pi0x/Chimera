//! What both of `cs_source`'s test binaries share (`mod common;` in
//! tests/it/main.rs (a copy of `cs_source`'s tests/common/mod.rs); a change here runs all of the
//! crate's tests, `repo_checks`' `affected`).
//!
//! `load_map`: a CS:S map by id from the user's install, as the CS:S app
//! loads it (`cs_source_app::games::load_map_level`, which tests here
//! can't use: the app sits above this crate); `load_map_file` from a
//! downloaded copy and `map_file_bytes` (the file the network offers and
//! hashes), as the app's `games` has them. `stack.rs`: what the scenario
//! tests run on, at the app's paths.

use chimera_world::map::MapData;

/// The install's mount for a `cs_source:<name>` id, and the name.
fn mount_of(id: &str) -> Result<(chimera_core::mount::Mount, &str), String> {
    chimera_core::mount::offline::allow_installs(); // reads the machine's install (`mount::offline`)
    let name = id
        .strip_prefix("cs_source:")
        .ok_or_else(|| format!("map ID `{id}` should look like cs_source:name"))?;
    let install = cs_source::game::INFO
        .install()
        .ok_or("no `cs_source` install found (chimera_install <folder>, or chimera.local.toml)")?;
    let mount = cs_source::game::mount::open(&install).map_err(|e| e.to_string())?;
    Ok((mount, name))
}

/// Map `id` (`cs_source:<name>`) from the install (chimera.local.toml,
/// saved settings or Steam).
#[allow(dead_code)]
pub fn load_map(id: &str) -> Result<MapData, String> {
    load_map_level(id, 0)
}

/// `load_map` at a Source `mat_hdr_level` (0 LDR, 1 bloom, 2 HDR).
pub fn load_map_level(id: &str, hdr_level: u8) -> Result<MapData, String> {
    let (mount, name) = mount_of(id)?;
    cs_source::game::bsp::load_level(&mount, name, hdr_level)
}

/// `load_map_level` with `options`.
pub fn load_map_with(id: &str, options: &chimera_world::map::MapLoadOptions) -> Result<MapData, String> {
    let (mount, name) = mount_of(id)?;
    cs_source::game::bsp::load_level_with(&mount, name, options)
}

/// `load_map` without textures (`MapLoadOptions::lean`): for tests that
/// never draw the map or read its texels (screenshot, drawn-part and
/// texture tests load it whole).
#[allow(dead_code)]
pub fn load_map_lean(id: &str) -> Result<MapData, String> {
    load_map_with(id, &chimera_world::map::MapLoadOptions::lean())
}

/// `load_map_level` without textures.
#[allow(dead_code)]
pub fn load_map_level_lean(id: &str, hdr_level: u8) -> Result<MapData, String> {
    let options = chimera_world::map::MapLoadOptions {
        hdr_level: Some(hdr_level),
        textures: false,
        ..Default::default()
    };
    load_map_with(id, &options)
}

/// `load_map_level` from a file of the map (a downloaded copy), its
/// content from the install.
#[allow(dead_code)]
pub fn load_map_file(id: &str, file: &std::path::Path, hdr_level: u8) -> Result<MapData, String> {
    let (mount, name) = mount_of(id)?;
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    cs_source::game::bsp::load_level_bytes(&mount, name, bytes, hdr_level)
}

/// The bytes of map `id`'s file as `load_map` reads it, for the network's
/// map offer and hash checks (`net::maps::MapFiles`). None if there's none.
#[allow(dead_code)]
pub fn map_file_bytes(id: &str) -> Option<Vec<u8>> {
    let (mount, name) = mount_of(id).ok()?;
    mount.read(&format!("maps/{name}.bsp")).ok()
}

/// A character that stays where it is put and can be hurt: noclip's
/// movement slot without its marker, so no movement runs and damage isn't
/// refused (a noclipping character takes none, `core::Noclip`). The
/// victims of the ragdoll and drawn-part tests.
#[allow(dead_code)]
pub fn spawn_still(sim: &mut chimera_world::harness::Sim, at: bevy::math::Vec3) -> bevy::ecs::entity::Entity {
    let p = sim.spawn_character(at, chimera_sim::movement::noclip::ID);
    sim.app.world_mut().entity_mut(p).remove::<chimera_core::core::Noclip>();
    p
}

/// The scenario tests' usual `Sim`: CS:S's base (`CsSim`), the greybox map
/// and CS:S's weapons, at CS:S's tick (0.015 s). Per-test variants (a
/// seeded bot config, stand-in materials) start from it.
#[allow(dead_code)]
pub fn sim() -> chimera_world::harness::Sim {
    use cs_source::{game::TICK_INTERVAL, game::weapons::CsWeaponsPlugin, harness::CsSim};
    let mut sim = chimera_world::harness::Sim::new(CsSim, (chimera_world::greybox::GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}
