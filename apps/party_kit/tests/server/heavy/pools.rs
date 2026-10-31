//! Every map a pool names exists in the install it comes from: the Combat
//! Arms parts' pool, CS:S's, the parts pool Ultimate Gun Game builds from,
//! and each `maps = [...]` list of `minigames/*.toml`. A Combat Arms name is
//! a `worlds*/<name>.dat` of the install's archives (`world::find`), a CS:S
//! one a `maps/<name>.bsp` in its mount. Passes with a note for a game without
//! an install (or served by the asset store alone); only file names are listed, nothing is read or written.

use chimera_core::mount::config::LocalConfig;

/// Every `game:map` named by a pool, with where it is named.
fn pools() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in [&cs_source::game::parts::PARTS, &combat_arms::game::parts::PARTS] {
        out.extend(part.maps.iter().map(|m| (m.to_string(), format!("{} parts", part.game))));
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../minigames");
    for file in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()) {
        if file.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let doc: toml::Table = std::fs::read_to_string(&file).unwrap().parse().unwrap();
        let maps = doc.values().filter_map(|v| v.as_table()).chain([&doc]).filter_map(|t| t.get("maps")?.as_array());
        for m in maps.flatten().filter_map(|m| m.as_str()) {
            out.push((m.to_string(), file.file_name().unwrap().to_string_lossy().into_owned()));
        }
    }
    out
}

#[test]
fn every_map_in_every_pool_resolves() {
    let config = LocalConfig::load().ok();
    // The asset store alone holds only the sets packed so far: no
    // install to check the pools against.
    let info = &combat_arms::game::INFO;
    let ca = info.mount().filter(|_| !info.served_partial());
    // The install's maps, or the asset store's standing in for it.
    let css = config
        .as_ref()
        .and_then(|c| c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref()))
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike/maps"))
        .and_then(|_| cs_source::game::INFO.mount());
    if ca.is_none() {
        eprintln!("note: no Combat Arms install, its pools are not checked");
    }
    if css.is_none() {
        eprintln!("note: no CS:S install, its pools are not checked");
    }
    let mut missing = Vec::new();
    for (id, from) in pools() {
        let (game, name) = id.split_once(':').unwrap_or(("cs_source", &id));
        match game {
            "combat_arms" => {
                if let Some(m) = &ca
                    && combat_arms::game::world::find(m, name).is_none()
                {
                    missing.push(format!("{id} ({from})"));
                }
            }
            "cs_source" => {
                if let Some(m) = &css
                    && !m.contains(&format!("maps/{name}.bsp"))
                {
                    missing.push(format!("{id} ({from})"));
                }
            }
            other => missing.push(format!("{id} ({from}): unknown game {other}")),
        }
    }
    assert!(missing.is_empty(), "pool maps that don't resolve: {missing:?}");
}
