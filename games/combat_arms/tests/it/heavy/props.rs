//! Combat Arms' map props and worn attachments from the user's install
//! (`game::props`, `characters::records`): the capture-the-flag maps'
//! flags drawn in their objects' render styles, a model placed with a
//! render-style list drawn in it, and the bots' pool of goggles and
//! backpacks readable (models per figure type and skins). Passes with a
//! note without the install or the keys.

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapAlpha;
use combat_arms::game::{
    GAME, INFO,
    model, movement, props,
    render_style::{self, Style},
    world,
};
use lithtech_formats::world::World;

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

fn has(key: &str) -> bool {
    combat_arms::game::mount::keys().is_ok_and(|k| k.contains_key(key))
}

/// BATTLESHIP's flags (`FlagItem`, V2-keyed model) are placed with the
/// render styles their objects list (`RenderStyle0`: `RS\EdgeAmp01.ltb`),
/// each piece by its render-style index.
#[test]
fn ctf_flags_in_their_render_styles() {
    let Some(mount) = mount() else { return };
    if !has("V2") {
        eprintln!("skipping: the flags' model needs the V2 key");
        return;
    }
    let map = world::load_world(&mount, "battleship").unwrap();
    let bytes = mount.read(&world::find(&mount, "battleship").unwrap()).unwrap();
    let w = World::parse(&bytes).unwrap();
    let file = |p: &str| mount.read(p).ok();
    let flags: Vec<_> = w.objects.iter().filter(|o| o.class == "FlagItem").collect();
    assert!(!flags.is_empty());
    let styles = props::render_styles(flags[0]);
    assert_eq!(styles[0], "RS\\EdgeAmp01.ltb");
    let loaded = model::load(props::model_file(flags[0]).unwrap(), &file).unwrap();
    let want: Vec<Style> = model::drawn_lods(&loaded.model)
        .iter()
        .map(|l| render_style::of_list(&styles, l.render_style))
        .collect();
    eprintln!("flag pieces' styles: {want:?}");
    let placed: Vec<_> = map
        .props
        .iter()
        .filter(|p| p.entity.is_some_and(|e| map.entities[e].classname() == "FlagItem"))
        .collect();
    assert_eq!(placed.len(), flags.len());
    for p in placed {
        let m = &map.models[p.model];
        assert_eq!(m.meshes.len(), want.len());
        for (mesh, style) in m.meshes.iter().zip(&want) {
            let mut expect = chimera_world::map::MapMesh::default();
            render_style::apply(&mut expect, *style);
            assert_eq!(mesh.unlit, expect.unlit);
            if expect.alpha != MapAlpha::Opaque {
                assert_eq!(mesh.alpha, expect.alpha);
            }
        }
    }
}

/// A model placed with a render-style list draws every piece in it:
/// SNOWSTORM's waving flag (unkeyed) added and unlit, then as glass.
#[test]
fn placed_model_in_a_render_style() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let mut map = world::load_world(&mount, "snowstorm").unwrap();
    let bytes = mount.read(&world::find(&mount, "snowstorm").unwrap()).unwrap();
    let w = World::parse(&bytes).unwrap();
    let scale = movement::movement_data().meters_per_unit;
    for (styles, alpha, unlit) in [
        ("RS\\additive.ltb", MapAlpha::Add, true),
        ("RS\\glass.ltb", MapAlpha::Blend, false),
    ] {
        props::dev_prop(
            &w,
            &mut map,
            &format!("fx/lv_used/snowstorm/ss_flag01.ltb, fx/lv_used/snowstorm/ss_flag.dtx; {styles}"),
            scale,
            &file,
        );
        let m = &map.models[map.props.last().unwrap().model];
        assert!(!m.meshes.is_empty());
        for mesh in &m.meshes {
            assert_eq!((mesh.alpha, mesh.unlit), (alpha, unlit), "{styles}");
            assert!(mesh.texture.is_some());
        }
    }
}

/// The bots' goggles and backpacks (data/characters.toml `[loadouts]`)
/// are records of the install's attachments file whose models (per
/// figure type) and skins read.
#[test]
fn pool_attachments_read() {
    let Some(mount) = mount() else { return };
    if !(has("V1") && has("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let file = |p: &str| mount.read(p).ok();
    let ca = combat_arms::game::data::CaData::install();
    let d = ca.characters();
    let mut failed = Vec::new();
    for (kind, pool, list) in [("goggle", &d.loadouts.goggles, &d.goggle), ("backpack", &d.loadouts.backpacks, &d.backpack)] {
        for &id in pool.iter().filter(|&&id| id >= 0) {
            let Some(r) = list.iter().find(|r| r.id == id) else {
                failed.push(format!("{kind} {id}: no record"));
                continue;
            };
            eprintln!("{kind} {id}: {} on {} {:?} skins {:?}", r.name, r.socket, r.files, r.skins);
            assert_eq!(r.socket, if kind == "goggle" { "Eyes" } else { "1W_N" });
            for f in r.files.iter().take(2) {
                if let Err(e) = model::load_own(f, &file) {
                    failed.push(format!("{kind} {id}: {e}"));
                }
            }
            for s in r.skins.iter().filter(|s| !s.is_empty()) {
                if mount.read(s).is_err() {
                    failed.push(format!("{kind} {id}: skin {s} doesn't read"));
                }
            }
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}
