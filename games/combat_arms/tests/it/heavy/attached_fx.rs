//! The view models' attached effects in the user's install
//! (specs/combat_arms/view_models.md 3, case 44; `view_model::attached`):
//! which weapons name `PVAttachFXName#` groups, what those groups hold
//! in the effect file, and the keys of the chainsaws' swings. Prints a
//! census (`--nocapture`); checks the XK-2's five lights and that every
//! named group is in the effect file. Needs the V1 key (the weapons
//! file); passes with a note without it. Nothing read is written.

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{
    GAME, INFO,
    data::{attributes::WeaponRecord, open_attributes},
    effects::library::FX_FILE,
    model,
};
use lithtech_formats::fxf::{FxFile, FxValue};

fn install() -> Option<(chimera_core::mount::Mount, Vec<WeaponRecord>)> {
    let Some(path) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    let rez = path.join("Game").join("ATTRIBUTES.rez");
    // The archive, or its files through the asset store standing in for
    // the install (decrypted there).
    let attrs = if rez.is_file() {
        open_attributes(&rez)
    } else if INFO.served_alone() {
        combat_arms::game::data::attributes_from_mount(&INFO.mount()?)
    } else {
        eprintln!("skipping: no {}", rez.display());
        return None;
    };
    let Ok(attrs) = attrs else {
        eprintln!("skipping: the attribute files don't open (no V1 key)");
        return None;
    };
    let weapons = attrs.weapons?.weapons;
    Some(((INFO.open)(&path).ok()?, weapons))
}

fn value(v: &FxValue) -> String {
    format!("{v:?}")
}

#[test]
fn attached_effects_census() {
    let Some((mount, weapons)) = install() else { return };
    let fx = FxFile::parse(&String::from_utf8_lossy(&mount.read(FX_FILE).unwrap())).unwrap();
    let with: Vec<&WeaponRecord> = weapons.iter().filter(|w| !w.pv_attach_fx.is_empty()).collect();
    eprintln!("{} of {} weapons have attached effects", with.len(), weapons.len());
    let mut groups = std::collections::BTreeSet::new();
    for w in &with {
        eprintln!(
            "  {:28} always {} fx {:?} model {}",
            w.name, w.pv_attach_fx_always, w.pv_attach_fx, w.pv_model
        );
        groups.extend(w.pv_attach_fx.iter().cloned());
    }
    let mut missing = Vec::new();
    for g in &groups {
        let Some(group) = fx.group(g) else {
            missing.push(g.clone());
            continue;
        };
        eprintln!("group {} phase {} ms, {} keys", group.name, group.phase_ms, group.keys.len());
        for k in &group.keys {
            let props: Vec<String> = k
                .props
                .iter()
                .filter(|p| {
                    [
                        "UpdatePos", "AttachName", "Offset", "Sound", "Loop", "PlayLocal", "InnerRadius", "OuterRadius",
                        "Volume", "Texture", "Sprite", "EmissionInterval", "Facing", "DisableZ", "Model",
                    ]
                    .iter()
                    .any(|n| p.name.eq_ignore_ascii_case(n))
                })
                .map(|p| format!("{}={}", p.name, value(&p.value)))
                .collect();
            eprintln!("    {} [{}..{}] {}", k.kind, k.start, k.end, props.join(" "));
        }
    }
    // The chainsaws' sounds and swing keys.
    for w in weapons.iter().filter(|w| w.name.to_ascii_lowercase().contains("chainsaw")) {
        eprintln!(
            "{}: fire {:?} alt {:?} reload {:?} select {:?} misc {:?} fx {:?}",
            w.name, w.fire_snd, w.alt_fire_snd, w.reload_snd, w.select_snd, w.misc_sounds, w.pv_attach_fx
        );
        for f in std::iter::once(&w.fire_snd).chain(&w.reload_snd).filter(|f| !f.is_empty()) {
            if let Some(c) = combat_arms::game::effects::library::sound_clip(f, &mount) {
                let secs = c.samples.len() as f32 / c.channels.max(1) as f32 / c.rate.max(1) as f32;
                eprintln!("    {f}: {secs:.3} s");
            }
        }
        let file = |p: &str| mount.read(p).ok();
        match model::load(&w.pv_model, &file) {
            Ok(m) => {
                for e in m.list.entries.iter().filter(|e| e.animation.keys.iter().any(|k| !k.string.is_empty())) {
                    let keys: Vec<String> = e.animation.keys.iter().filter(|k| !k.string.is_empty()).map(|k| format!("{} {:?}", k.time, k.string)).collect();
                    eprintln!("    {} {} ms: {}", e.name(), e.length(), keys.join(", "));
                }
            }
            Err(e) => eprintln!("    view model: {e}"),
        }
    }
    // The XK-2's five lights aren't in the Steam install's effect file:
    // they play nothing (effects_sounds.md: a missing group plays nothing).
    eprintln!("groups missing from the effect file: {missing:?}");
    for g in &missing {
        assert!(g.to_ascii_lowercase().starts_with("timebomb"), "only the time bomb's lights are missing: {g}");
    }
    let xk2 = weapons.iter().find(|w| w.name.eq_ignore_ascii_case("XK-2")).or_else(|| {
        weapons.iter().find(|w| w.pv_attach_fx.iter().any(|g| g.eq_ignore_ascii_case("TimebombGreen")) && w.pv_attach_fx.len() == 5)
    });
    if let Some(x) = xk2 {
        let want = ["TimebombGreen", "TimebombRed", "TimebombYellow1", "TimebombYellow2", "TimebombYellow3"];
        assert_eq!(x.pv_attach_fx.len(), 5, "{}: five lights", x.name);
        for (got, want) in x.pv_attach_fx.iter().zip(want) {
            assert!(got.eq_ignore_ascii_case(want), "{}: {got} is {want}", x.name);
        }
    } else {
        eprintln!("note: no XK-2 record in this install");
    }
}
