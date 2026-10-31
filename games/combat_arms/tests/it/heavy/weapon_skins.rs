//! Every Combat Arms weapon's textures from the user's install: for each
//! weapon of the merged set (ours and the install's), its view model
//! (`[models] view`, `view_skins`) and held model (`[models] world`, its
//! look's skins), each drawn piece's skin as the loaders resolve it
//! (`game::model::drawn_lods`, its first texture slot, `world::model_textures`'
//! reading: a `.dtx`, or a sprite's frames), grouped by why a piece
//! would draw untextured (grey): no skin entry for its slot, a file that
//! isn't there or is locked (a keyed archive without its key), one that
//! doesn't decode; and what else may make it look wrong: a second texture
//! slot (drawn under a layered style, not under a single-texture one), a
//! render style our table doesn't know (drawn opaque, lit). Sprite skins
//! are counted by whether they animate (two or more frames resolve).
//!
//! Needs the V1 key (the weapons file) and the V2 key (the rifles',
//! SMGs', snipers' and third-person archives) in chimera.local.toml; run
//! `cargo test -p combat_arms --features dev,ui --test it
//! heavy::weapon_skins -- --nocapture` for the report (`ui`: with the
//! names the game shows); `CHIMERA_SKINS_SHOW=m416,sig550` also prints
//! every piece of the weapons named so (slots, skins, render styles). Skips with a note
//! without the install or the weapons file.

use std::collections::BTreeMap;

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{GAME, INFO, data::CaData, effects::library::sprite_texture, model, render_style};
use lithtech_formats::{dtx::Dtx, spr::Sprite};

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

/// How one skin entry resolves: a texture, or a sprite skin with how
/// many of its frames resolve (it animates with two or more).
fn resolve(entry: &str, file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Option<(usize, usize)>, String> {
    if entry.is_empty() {
        return Err("no skin entry for the piece's slot".into());
    }
    let path = model::normalize(entry);
    let texture = |path: &str| -> Result<(), String> {
        let bytes = file(path).ok_or("texture missing or locked")?;
        Dtx::parse(bytes).map(|_| ()).map_err(|e| format!("texture doesn't decode: {e}"))
    };
    if path.ends_with(".spr") {
        let bytes = file(&path).ok_or("sprite missing or locked")?;
        let Ok(sprite) = Sprite::parse(&bytes) else {
            // Drawn from its first frame (`world::textures`).
            texture(&sprite_texture(&bytes).ok_or("sprite unreadable")?)?;
            return Ok(Some((1, 1)));
        };
        let ok = sprite.frames.iter().filter(|f| texture(&model::normalize(f)).is_ok()).count();
        if ok == 0 {
            return Err("no frame of the sprite resolves".into());
        }
        return Ok(Some((ok, sprite.frames.len())));
    }
    texture(&path).map(|_| None)
}

/// The weapons' names as the game shows them (with the `ui` feature: the
/// install's string table), by id.
fn display_names(mount: &chimera_core::mount::Mount) -> std::collections::HashMap<String, String> {
    #[cfg(feature = "ui")]
    {
        use combat_arms::ui::install;
        if let Some(strings) = install::strings(mount, &install::language()) {
            return install::weapon_names(mount, &strings).into_iter().collect();
        }
    }
    let _ = mount;
    Default::default()
}

#[test]
fn every_weapons_skins_resolve() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's weapons file isn't readable (the V1 key)");
        return;
    }
    let looks = ca.characters();
    let names = display_names(&mount);
    // `CHIMERA_SKINS_SHOW=a,b`: every piece of the weapons whose name has
    // one of these (any case).
    let show: Vec<String> = std::env::var("CHIMERA_SKINS_SHOW")
        .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    // cause -> weapons (install name, which model).
    let mut causes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut count = |cause: String, what: String| causes.entry(cause).or_default().push(what);
    let mut weapons = 0;
    let mut animated = 0;
    for w in &ca.weapon_set().weapons {
        let Some(record) = ca.install_weapon(&w.id) else { continue };
        weapons += 1;
        let look = looks.weapon(Some(&w.id));
        let models = [
            ("view", w.models.view.as_deref(), &w.models.view_skins, &record.pv_render_styles),
            ("held", w.models.world.as_deref(), &look.skins, &look.render_styles),
        ];
        for (which, path, skins, styles) in models {
            let what = match names.get(&w.id) {
                Some(shown) => format!("{} \"{shown}\" ({which})", record.name),
                None => format!("{} ({which})", record.name),
            };
            let Some(path) = path else {
                count("no model in the data".into(), what);
                continue;
            };
            let loaded = match model::load(path, &file) {
                Ok(l) => l,
                Err(e) => {
                    let cause = if e.contains("locked") { "model missing or locked" } else { "model doesn't load" };
                    count(cause.into(), format!("{what}: {e}"));
                    continue;
                }
            };
            let show = show.iter().any(|s| what.to_lowercase().contains(s));
            if show {
                println!("{what}: {path}; skins {skins:?}; styles {styles:?}");
            }
            for lod in model::drawn_lods(&loaded.model) {
                let slots = lod.used_textures();
                if show {
                    println!("  piece: slots {slots:?}, style {}", lod.render_style);
                }
                let entry = slots
                    .first()
                    .and_then(|i| usize::try_from(*i).ok())
                    .and_then(|i| skins.get(i))
                    .cloned()
                    .unwrap_or_default();
                match resolve(&entry, &file) {
                    Ok(None) => {}
                    Ok(Some((ok, n))) if ok > 1 => {
                        animated += 1;
                        count("sprite skin, animated".into(), format!("{what}: {entry} ({ok} of {n} frames)"));
                    }
                    Ok(Some(_)) => count("sprite skin, one frame (still)".into(), format!("{what}: {entry}")),
                    Err(e) => count(format!("grey: {e}"), format!("{what}: slot {:?} -> {entry:?}", slots)),
                }
                let style = usize::try_from(lod.render_style).ok().and_then(|i| styles.get(i));
                let layered = style.is_some_and(|n| matches!(render_style::style(n), render_style::Style::Layered { .. }));
                if slots.len() > 1 {
                    let rest: Vec<String> = slots[1..]
                        .iter()
                        .map(|i| usize::try_from(*i).ok().and_then(|i| skins.get(i)).cloned().unwrap_or_default())
                        .collect();
                    let cause = if layered {
                        "a second texture slot, drawn as an environment or rim map"
                    } else {
                        "a second texture slot under a single-texture style (not drawn, as the game)"
                    };
                    count(cause.into(), format!("{what}: {rest:?}"));
                }
                if let Some(name) = style
                    && render_style::style(name) == render_style::Style::Opaque
                    && !name.to_ascii_lowercase().ends_with("default.ltb")
                {
                    count(format!("render style drawn as the default: {name}"), what.clone());
                }
            }
        }
    }
    // Shown names asked for whose weapon isn't in the set (skipped by
    // the merge: no template of its class, or a name whose id another
    // record took).
    for (id, shown) in &names {
        if show.iter().any(|s| shown.to_lowercase().contains(s)) && ca.weapon_set().get(id).is_none() {
            println!("{id} \"{shown}\": not in the weapon set");
        }
    }
    println!("{weapons} install weapons");
    for (cause, list) in &causes {
        println!("\n{cause}: {}", list.len());
        for w in list {
            println!("  {w}");
        }
    }
    assert!(weapons > 0, "the weapons file names weapons");
    // models_ltb.md "Sprite skins": the sprite-skinned pieces play their
    // frames (310 pieces on the dev box's install, 2026-10-30).
    println!("\n{animated} pieces with an animated sprite skin");
    assert!(animated > 0, "no weapon piece has an animated sprite skin");
    // The playtest's grey "Prismatic" M416: a sprite skin was dropped.
    for w in &ca.weapon_set().weapons {
        let Some(record) = ca.install_weapon(&w.id) else { continue };
        for s in record.pv_skins.iter().filter(|s| s.to_ascii_lowercase().ends_with(".spr")) {
            assert!(w.models.view_skins.contains(&model::normalize(s)), "{}: {s} kept", record.name);
        }
    }
}
