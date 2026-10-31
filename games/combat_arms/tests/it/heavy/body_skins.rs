//! Every body a ladder rung may dress its players in
//! (`CharacterData::rung_bodies`: the install's player bodies,
//! specialists and mercenaries with them) from the user's install: each
//! drawn piece of its body file (`game::model::drawn_lods`, its first
//! texture slot) resolved as the body loader fills it (characters.md 2:
//! 0 upper and 1 legs at camouflage 0, 2 the team mark, 3 and 4 mark 0,
//! 5 on the record's `AdditionalSkins#`; a `.dtx`, or a sprite's first
//! frame), grouped by why a piece would draw untextured (grey): no skin
//! entry for its slot, a file that isn't there or is locked, one that
//! doesn't decode, a body file that doesn't load; and what else may make
//! it look wrong: a second texture slot we don't draw, a render style our
//! table doesn't know (drawn opaque, lit).
//!
//! Needs the V1 key (the models file) and the V2 key (`CHARS_M_BODY`);
//! run `cargo test -p combat_arms --features dev --test it
//! heavy::body_skins -- --nocapture` for the report;
//! `CHIMERA_SKINS_SHOW=ghillie,ice` also prints every piece of the bodies
//! named so; `CHIMERA_BODIES_LOAD=1` also loads each body the way a rung
//! does (`models::load_body`, faces and head pieces too, minutes) and
//! lists its meshes left untextured. Skips with a note without the
//! install or the models file.

use std::collections::BTreeMap;

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{GAME, INFO, characters::models, data::CaData, effects::library::sprite_texture, model, render_style};
use lithtech_formats::dtx::Dtx;

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !install.join("Game").is_dir() {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// How one skin entry resolves.
fn resolve(entry: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<&'static str, String> {
    if entry.is_empty() {
        return Err("no skin entry for the piece's slot".into());
    }
    let mut path = model::normalize(entry);
    let mut sprite = false;
    if path.ends_with(".spr") {
        let bytes = file(&path).ok_or("sprite missing or locked")?;
        path = sprite_texture(&bytes).ok_or("sprite unreadable")?;
        sprite = true;
    }
    let bytes = file(&path).ok_or("texture missing or locked")?;
    Dtx::parse(bytes).map_err(|e| format!("texture doesn't decode: {e}"))?;
    Ok(if sprite { "sprite (first frame only)" } else { "texture" })
}

#[test]
fn every_rung_bodys_skins_resolve() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's models file isn't readable (the V1 key)");
        return;
    }
    let data = ca.characters();
    let show: Vec<String> = std::env::var("CHIMERA_SKINS_SHOW")
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    // cause -> pieces (record id, name, file).
    let mut causes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut count = |cause: String, what: String| causes.entry(cause).or_default().push(what);
    let bodies = data.rung_bodies();
    for &id in &bodies {
        let r = data.model(id).unwrap();
        let what = format!("{id} {} ({})", r.name, r.file);
        let loaded = match model::load(&r.file, &file) {
            Ok(l) => l,
            Err(e) => {
                let cause = if !mount.contains(&model::normalize(&r.file)) {
                    "grey: body file not in the install"
                } else if e.contains("locked") {
                    "grey: body file locked"
                } else {
                    "grey: body file doesn't load"
                };
                count(cause.into(), format!("{what}: {e}"));
                continue;
            }
        };
        let show = show.iter().any(|s| what.to_lowercase().contains(s));
        if show {
            println!("{what}: upper {:?}; lower {:?}; additional {:?}; styles {:?}", r.upper_skins, r.lower_skins, r.additional_skins, r.render_styles);
        }
        let (upper, lower) = data.skins(r, 0);
        for lod in model::drawn_lods(&loaded.model) {
            let slots = lod.used_textures();
            let s = slots.first().copied().unwrap_or(-1);
            let entry = match s {
                0 => upper.unwrap_or_default().to_string(),
                1 => lower.unwrap_or_default().to_string(),
                2 => data.team_mark(false, false).to_string(),
                3 | 4 => data.marks.enemy.clone(),
                s => data.additional_skin(r, s).unwrap_or_default().to_string(),
            };
            let style = usize::try_from(lod.render_style).ok().and_then(|i| r.render_styles.get(i)).cloned().unwrap_or_default();
            if show {
                println!("  piece: slots {slots:?}, style {} {style} -> {entry:?}", lod.render_style);
            }
            match resolve(&entry, &file) {
                Ok("texture") => {}
                Ok(kind) => count(kind.into(), format!("{what}: {entry}")),
                Err(e) => count(format!("grey: {e}"), format!("{what}: slot {s} -> {entry:?}")),
            }
            if slots.len() > 1 {
                count("a second texture slot (not drawn)".into(), format!("{what}: slots {slots:?}"));
            }
            if !style.is_empty()
                && render_style::style(&style) == render_style::Style::Opaque
                && !style.to_ascii_lowercase().ends_with("default.ltb")
            {
                count(format!("render style drawn as the default: {style}"), what.clone());
            }
        }
    }
    // The bodies as a rung loads them: every mesh (body, faces, head
    // pieces, goggles, backpacks) left without a texture.
    if std::env::var("CHIMERA_BODIES_LOAD").is_ok() {
        for &id in &bodies {
            let name = &data.model(id).unwrap().name;
            match models::load_body(id, &file) {
                Ok((c, _)) => {
                    for (k, look) in c.model.skins[0].iter().enumerate().filter(|(_, l)| l.texture.is_none()) {
                        let part = c.model.meshes[k].body;
                        count("loaded: a mesh left untextured".into(), format!("{id} {name}: mesh {k} (part {part:?}) {:?}", look.material));
                    }
                }
                Err(e) => count("loaded: doesn't load".into(), format!("{id}: {e}")),
            }
        }
    }
    println!("{} rung bodies", bodies.len());
    for (cause, list) in &causes {
        println!("\n{cause}: {}", list.len());
        for w in list {
            println!("  {w}");
        }
    }
    assert!(bodies.len() > 6, "the install adds bodies to the rungs' pool");
    // The playtest's grey ghillie leaves: slot 5, the record's
    // `AdditionalSkins0`, had no entry.
    let none = causes.get("grey: no skin entry for the piece's slot").map_or(0, Vec::len);
    assert_eq!(none, 0, "pieces whose slot names no skin: {:?}", causes.get("grey: no skin entry for the piece's slot"));
    let ghillie = bodies.iter().filter_map(|&i| data.model(i)).find(|m| m.file.to_lowercase().contains("ghillie"));
    if let Some(g) = ghillie {
        assert!(g.additional_skins.first().is_some_and(|s| s.to_lowercase().contains("ghillie")), "{}: {:?}", g.name, g.additional_skins);
    }
}

/// models_ltb.md R9 on the install's records: `Face160` (Alpha Gear)
/// fills slot 1 with the reflection `textures\128xCharSkin.dtx` (its
/// whole list in order); `Hair331` (Summer Claire) at camouflage 1 fills
/// slot 0 with its `Skin1` and leaves slot 1 empty.
#[test]
fn r9_piece_skins_from_the_records() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's models file isn't readable (the V1 key)");
        return;
    }
    let data = ca.characters();
    let face = data.face(160).expect("Face160");
    let reflection = render_style::piece_skin(render_style::Piece::Face, &face.skins, 1, 0).expect("slot 1");
    assert_eq!(model::normalize(reflection), "textures/128xcharskin.dtx", "{:?}", face.skins);
    let hair = data.head.iter().find(|h| h.id == 331).expect("Hair331");
    assert_eq!(
        render_style::piece_skin(render_style::Piece::Head, &hair.skins, 0, 1),
        hair.skins.get(1).map(String::as_str),
        "slot 0 = Skin1 at camouflage 1: {:?}",
        hair.skins
    );
    assert_eq!(render_style::piece_skin(render_style::Piece::Head, &hair.skins, 1, 1), None, "slot 1 is never filled");
}
