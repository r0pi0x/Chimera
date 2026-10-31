//! Combat Arms' effects and sounds from the user's install
//! (`game::effects`, specs/combat_arms/effects_sounds.md): the sound and
//! effect archives read without keys, every sound the tables and weapons
//! name decodes, the effect file parses and has the groups they name (the
//! C and Red silencers' excepted, Edge cases), their textures decode, and
//! the numbers the effect file holds for test cases 21, 24, 32, 34 and 35.
//! Then a map: its surfaces typed, the materials registered, and a shot
//! in play making a decal and the burst's particles. Passes with a note
//! without `[games.combat_arms] path` in chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{
    core::{Health, LocalPlayer},
    mount::config::LocalConfig,
};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin, particles::Particles},
};
use combat_arms::{
    game::{
        GAME, INFO,
        data::CaData,
        effects::{
            data::{CaFxTables, CaWeaponFx, EFFECTS},
            library::{self, FX_FILE},
            rules::CaMarks,
        },
        movement,
        view_model::{CaViewParams, VIEW},
        world,
    },
    harness::CaSim,
};
use lithtech_formats::{
    fxf::{FxFile, FxKey, sample_keys},
    rez::{RezArchive, RezKeys},
};

fn install() -> Option<std::path::PathBuf> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some(install)
}

fn mount() -> Option<chimera_core::mount::Mount> {
    Some((INFO.open)(&install()?).unwrap())
}

/// The archives the effects read: none keyed.
#[test]
fn sound_and_effect_archives_read_without_keys() {
    let Some(install) = install() else { return };
    let game = install.join("Game");
    if !game.is_dir() {
        eprintln!("skipping: checks the install's own archives (served from the asset store here)");
        return;
    }
    let mut names: Vec<String> = std::fs::read_dir(&game)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let u = n.to_uppercase();
            u.ends_with(".REZ")
                && (u.starts_with("GUNS_SND_")
                    || ["SND.REZ", "CHARS_SND.REZ", "UI_SND.REZ", "CLIENTFX.REZ", "FX.REZ", "GUNS_SC.REZ", "SPRITES.REZ"]
                        .contains(&u.as_str()))
        })
        .collect();
    names.sort();
    assert_eq!(names.len(), 20, "{names:?}");
    for n in &names {
        let a = RezArchive::open(game.join(n), &RezKeys::new()).unwrap();
        assert!(!a.locked(), "{n} is keyed");
    }
    eprintln!("unkeyed: {}", names.join(", "));
}

/// The weapons' effect hooks.
fn weapons() -> Vec<CaWeaponFx> {
    CaData::install().weapon_set().weapons.iter().filter_map(|w| w.hook(EFFECTS)).collect()
}

/// Every sound the tables name reads and decodes; every sound a weapon
/// names decodes when the install has it. The install's own weapon
/// records name files it doesn't ship (184 in build 26236938: deselect,
/// dry-fire and third reload sounds of many guns, the RMS12's
/// `GUNS_SND_LAUNCHER\AK47\*`, the emotes'): those play nothing, as in
/// the game (effects_sounds.md: a sound that doesn't load is silent), so
/// they are counted, not failed.
#[test]
fn every_named_sound_decodes() {
    let Some(mount) = mount() else { return };
    let t = CaFxTables::built_in();
    let tables: Vec<String> = t.sound_files().into_iter().map(str::to_string).collect();
    let mut files = tables.clone();
    for w in weapons() {
        files.extend(w.sound_files().map(str::to_string));
    }
    files.sort();
    files.dedup();
    let (mut absent, mut bad) = (Vec::new(), Vec::new());
    for f in &files {
        if library::sound_clip(f, &mount).is_some() {
            continue;
        }
        // Not in the install at all, and named only by its data: silent.
        if mount.read(f).is_err() && !tables.contains(f) {
            absent.push(f);
        } else {
            bad.push(f);
        }
    }
    eprintln!("{} sound files; {} named by the install's records but not in it", files.len(), absent.len());
    assert!(bad.is_empty(), "unreadable: {bad:?}");
    assert!(files.len() > 150);
    assert!(absent.len() < files.len() / 4, "most named sounds are there: {absent:?}");
}

fn fx_file(mount: &chimera_core::mount::Mount) -> FxFile {
    let bytes = mount.read(FX_FILE).unwrap();
    FxFile::parse(&String::from_utf8_lossy(&bytes)).unwrap()
}

/// The groups named exist (but the silencers' that the file lacks), and
/// their textures decode.
#[test]
fn effect_file_groups_and_textures() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    assert_eq!(fx.groups.len(), 530);
    let t = CaFxTables::built_in();
    let muzzles: Vec<String> = CaData::install().weapon_set()
        .weapons
        .iter()
        .filter_map(|w| w.hook::<CaViewParams>(VIEW))
        .map(|p| p.muzzle_fx)
        .collect();
    let wanted = library::wanted_groups(t, &weapons(), &muzzles, &fx);
    assert!(wanted.len() > 25, "{wanted:?}");
    // Every name in the tables resolves but the C and Red silencers'.
    let mut names: Vec<&str> = t.surface.iter().flat_map(|s| [&s.impact_fx, &s.grenade_fx]).map(String::as_str).collect();
    names.extend(t.impact.values().map(|i| i.fx.as_str()));
    names.extend(["AK47_PV_Muzz", "M4_HH_Muzz", "WEAPON_HH", "test", "Blood_Stain01"]);
    for n in names.into_iter().filter(|n| !n.is_empty()) {
        assert!(fx.group(n).is_some(), "{n}");
    }
    assert!(fx.group("Silenced01_PV_Muzz").is_none() && fx.group("Silenced01_HH_Muzz").is_none());
    let file = |p: &str| mount.read(p).ok();
    let mut textures = 0;
    for name in &wanted {
        for k in &fx.group(name).unwrap().keys {
            let Some(path) = library::key_texture(k, &file) else { continue };
            let dtx = lithtech_formats::dtx::Dtx::parse(mount.read(&path).unwrap_or_else(|e| panic!("{name}: {path}: {e}")));
            assert!(dtx.is_ok(), "{name}: {path}");
            textures += 1;
        }
    }
    assert!(textures > 50, "{textures}");
    // The decals' sprites the install has.
    for s in t.surface.iter().filter(|s| !s.mark.is_empty()) {
        if let Ok(b) = mount.read(&s.mark) {
            let tex = library::sprite_texture(&b).unwrap();
            assert!(mount.read(&tex).is_ok(), "{}: {tex}", s.mark);
        } else {
            eprintln!("{}: {} not in the install (no decal texture)", s.name, s.mark);
        }
    }
    // The casing models.
    for f in t.fire.values().filter(|f| f.shells) {
        let loaded = combat_arms::game::model::load(&f.shell_model, &file).unwrap();
        assert!(!loaded.model.pieces.is_empty(), "{}", f.shell_model);
        assert!(mount.read(&f.shell_skin).is_ok(), "{}", f.shell_skin);
    }
}

fn key<'a>(fx: &'a FxFile, group: &str, kind: &str, n: usize) -> &'a FxKey {
    fx.group(group)
        .unwrap()
        .keys
        .iter()
        .filter(|k| k.kind == kind)
        .nth(n)
        .unwrap_or_else(|| panic!("{group}: no {kind} #{n}"))
}

/// The largest scale key of a light and the group time it is reached.
fn light_peak(k: &FxKey) -> (f32, f32) {
    let keys = k.scale_keys();
    let (t, r) = keys.iter().copied().fold((0.0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    (r, k.start + t * (k.end - k.start))
}

/// Expected stains of a blood group: each splatting system's particles ×
/// its `SplatPercent`.
fn expected_stains(fx: &FxFile, group: &str) -> f32 {
    fx.group(group)
        .unwrap()
        .keys
        .iter()
        .filter(|k| k.kind == "ParticleSystem" && k.string("SplatEffect").is_some_and(|s| !s.is_empty()))
        .map(|k| k.int("ParticlesPerEmission").unwrap_or(0) as f32 * k.float("SplatPercent").unwrap_or(0.0) / 100.0)
        .sum()
}

#[test]
fn t20_t21_blood() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    let stains = expected_stains(&fx, "Impact_Body");
    assert!((stains - 1.0).abs() < 1e-3, "body {stains}");
    let stains = expected_stains(&fx, "Impact_Head");
    assert!((stains - 4.3).abs() < 0.05, "head {stains}");
    // The head's light: 300 u over 0.1 s. The file's keys run 300 -> 0
    // (the spec reads 0 -> 300; we play the file's).
    let l = key(&fx, "Impact_Head", "DynaLight", 0);
    assert_eq!(light_peak(l).0, 300.0);
    assert!((l.end - l.start - 0.1).abs() < 1e-4);
    assert!(!l.at_detail(0) && l.at_detail(2), "medium and high only");
    assert_eq!(light_peak(key(&fx, "Impact_Body", "DynaLight", 0)).0, 150.0);
    // The stain: 128 px at 0.25 (32 u), 3 s.
    let stain = fx.group("Blood_Stain01").unwrap();
    assert_eq!(stain.phase_ms, 3000);
    assert_eq!(stain.keys[0].scale_keys()[0].1, 0.25);
}

#[test]
fn t24_own_muzzle_numbers() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    let p = key(&fx, "AK47_PV_Muzz", "ParticleSystem", 0);
    let s = p.scale_keys();
    assert!((s[0].1 - 0.327).abs() < 1e-3 && (s.last().unwrap().1 - 0.5).abs() < 1e-3, "{s:?}");
    assert_eq!((p.float("MinParticleLifeSpan"), p.float("MaxParticleLifeSpan")), (Some(0.04), Some(0.05)));
    let l = key(&fx, "AK47_PV_Muzz", "DynaLight", 0);
    assert_eq!(light_peak(l).0, 170.0);
    assert!(!l.at_detail(0) && l.at_detail(1) && l.at_detail(2), "medium and high only");
    assert_eq!(fx.group("AK47_PV_Muzz").unwrap().phase_ms, 50);
    // `test`: the smoke puff and the 256 -> 170 light.
    let puff = key(&fx, "test", "ParticleSystem", 0);
    let s = puff.scale_keys();
    assert_eq!((s[0].1, s.last().unwrap().1), (4.0, 16.0));
    assert_eq!((puff.float("MinParticleLifeSpan"), puff.float("MaxParticleLifeSpan")), (Some(0.5), Some(0.8)));
    let l = key(&fx, "test", "DynaLight", 0).scale_keys();
    assert_eq!((l[0].1, l.last().unwrap().1), (256.0, 170.0));
}

#[test]
fn t32_frag_light_and_jitter() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    let (r, at) = light_peak(key(&fx, "Grenade_Stone", "DynaLight", 0));
    assert_eq!(r, 256.0);
    assert!((at - 0.199).abs() < 0.001, "{at}");
    let j = key(&fx, "Grenade_Stone", "CamJitter", 0);
    assert_eq!((j.float("InnerRadius"), j.float("OuterRadius")), (Some(200.0), Some(800.0)));
    assert_eq!(j.scale_keys()[0].1, 60.0);
    assert!(j.at_detail(0), "every detail");
}

#[test]
fn t34_smoke_cloud_numbers() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    let g = fx.group("Smoke_Grenade_01").unwrap();
    assert_eq!(g.phase_ms, 2000);
    let cloud = g
        .keys
        .iter()
        .filter(|k| k.kind == "ParticleSystem")
        .find(|k| k.float("MaxParticleLifeSpan") == Some(10.0))
        .unwrap();
    assert!(cloud.float("EmissionInterval").unwrap() <= 0.0, "one burst");
    assert_eq!(cloud.int("ParticlesPerEmission"), Some(10));
    assert_eq!(cloud.float("MinParticleLifeSpan"), Some(8.0));
    assert_eq!((cloud.float("MinRadius"), cloud.float("MaxRadius")), (Some(0.0), Some(10.0)));
    assert_eq!(cloud.vector("MinParticleVelocity"), Some([-1000.0, -1000.0, -200.0]));
    assert_eq!(cloud.vector("MaxParticleVelocity"), Some([1000.0, 1000.0, 400.0]));
    assert_eq!(cloud.float("GravityAcceleration"), Some(10.0));
    assert_eq!(cloud.float("Friction"), Some(0.98));
    assert_eq!(cloud.float("PercentToBounce"), Some(100.0));
    let s = cloud.scale_keys();
    assert_eq!((s[0].1, s.last().unwrap().1), (200.0, 300.0));
    // Opaque from 5.1 % of its life to 85.1 %.
    let ck = cloud.color_keys();
    let alpha: Vec<(f32, f32)> = ck.iter().map(|(t, c)| (*t, c[3])).collect();
    assert_eq!(sample_keys(&alpha, 0.0, 1.0), 0.0);
    assert!((sample_keys(&alpha, 0.052, 1.0) - 1.0).abs() < 0.05, "{alpha:?}");
    assert!((sample_keys(&alpha, 0.85, 1.0) - 1.0).abs() < 0.05, "{alpha:?}");
}

#[test]
fn t35_flash_look() {
    let Some(mount) = mount() else { return };
    let fx = fx_file(&mount);
    let l = key(&fx, "Flashbang_Ini", "DynaLight", 0);
    let (r, at) = light_peak(l);
    assert_eq!(r, 512.0);
    assert!((at - 0.127).abs() < 0.002, "{at}");
    assert!((l.end - 0.28).abs() < 0.002, "{}", l.end);
}

/// A map loads with its surfaces typed and the effects' materials; a shot
/// at its floor makes a decal and the burst's particles.
#[test]
fn shots_on_a_map() {
    let Some(mount) = mount() else { return };
    let map: MapData = world::load_from(&mount, "warehouse").unwrap();
    let typed = map.meshes.iter().filter(|m| m.surface.as_deref().is_some_and(|s| s != "0")).count();
    eprintln!("warehouse: {typed} of {} meshes typed, {} effect materials", map.meshes.len(), map.particles.materials.len());
    assert!(typed > map.meshes.len() / 2);
    assert!(map.particles.materials.len() > 30);
    // The refraction keys (effect shaders) aren't drawn as colour.
    let refract: Vec<&str> = map
        .particles
        .materials
        .iter()
        .map(|m| m.name.as_str())
        .filter(|n| n.contains("refract") || n.contains("heatsmoke"))
        .collect();
    assert!(refract.is_empty(), "{refract:?}");
    assert!(library::library().is_some());
    let spawn = map.spawns[0].0;
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(MapSimPlugin::new(map));
    });
    sim.set_tick_interval(combat_arms::game::TICK_INTERVAL);
    let p = sim.spawn_character(spawn + Vec3::Y * 0.1, movement::ID);
    sim.app.world_mut().entity_mut(p).insert((LocalPlayer, Health { current: 1.0, max: 1.0 }));
    sim.seconds(0.5);
    let w = chimera_sim::weapon::give(sim.app.world_mut(), p, "combat_arms:m4a1").unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<chimera_sim::weapon::Inventory>(p).unwrap().active, Some(w));
    sim.intent(p).pitch = -1.0;
    sim.ticks(2);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(3);
    let marks = sim.app.world().resource::<CaMarks>();
    eprintln!("decals: {:?}", marks.marks.iter().map(|m| &m.sprite).collect::<Vec<_>>());
    let particles = sim.app.world().resource::<Particles>().count();
    eprintln!("particles: {particles}");
    assert!(particles > 0, "the burst's and the muzzle's particles");
}

/// The `Male_Snd#` files of an autocast record `[section]` of the install's
/// autocastsound.txt (lower case, `/` separators).
fn autocast(text: &str, section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            inside = l == format!("[{section}]");
        } else if inside && l.starts_with("Male_Snd") {
            let file = l.split('"').nth(1).unwrap_or("").replace('\\', "/").to_lowercase();
            if !file.is_empty() {
                out.push(file);
            }
        }
    }
    out
}

/// Corrections 2026-10-22, 8 and 7: the local voices' files (male
/// template) and the Awareness perk's four sounds are the install's, and
/// each hurt kind's radius is 500; case 65 (files).
#[test]
fn c65_voice_files_are_the_installs() {
    let Some(mount) = mount() else { return };
    let text = String::from_utf8_lossy(&mount.read("attributes/autocastsound.txt").unwrap()).into_owned();
    let t = CaFxTables::built_in();
    assert_eq!(autocast(&text, "AutoCastLocalsound0"), t.sounds.dash, "BreathingDash");
    assert_eq!(autocast(&text, "AutoCastLocalsound1"), t.sounds.kill, "EnemyKill");
    assert_eq!(autocast(&text, "AutoCastLocalsound3"), t.sounds.idle, "Idle (empty for the male template)");
    assert_eq!(autocast(&text, "AutoCastLocalsound8"), t.sounds.heartbeat, "Heartbeat");
    for section in ["AutoCast3DSound0", "AutoCast3DSound1", "AutoCast3DSound2", "AutoCast3DSound6", "AutoCast3DSound10"] {
        let radius = text
            .lines()
            .skip_while(|l| l.trim() != format!("[{section}]"))
            .find(|l| l.starts_with("3DsoundRadius"))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse::<f32>().ok());
        assert_eq!(radius, Some(t.sounds.hurt_radius), "{section}");
    }
    let perks = String::from_utf8_lossy(&mount.read("attributes/perkskillinfo.txt").unwrap()).into_owned();
    let awareness: Vec<String> = perks
        .lines()
        .filter(|l| l.trim_start().starts_with("EffectSnd") && l.to_lowercase().contains("awareness"))
        .filter_map(|l| l.split('"').nth(1))
        .map(|f| f.replace('\\', "/").to_lowercase())
        .collect();
    assert_eq!(awareness, t.sounds.awareness);
    for f in t.sounds.kill.iter().chain(&t.sounds.dash).chain(&t.sounds.heartbeat).chain(&t.sounds.awareness) {
        assert!(library::sound_clip(f, &mount).is_some(), "{f} doesn't decode");
    }
}

/// Corrections 2026-10-22, 6; case 63 (install): the models file gives
/// the ghillie-type bodies a `FootprintSndVolume` of -0.5.
#[test]
fn c63_ghillie_models_have_the_share() {
    let Some(_) = install() else { return };
    let data = CaData::install();
    let models = data.attributes().and_then(|a| a.models.as_ref()).expect("the models file");
    let ghillie = models.bodies.iter().filter(|b| b.footprint_volume == -0.5).count();
    eprintln!("{ghillie} of {} bodies with -0.5", models.bodies.len());
    assert!(ghillie > 0, "some bodies are ghillie-type");
    assert!(models.bodies.iter().all(|b| b.footprint_volume == 0.0 || b.footprint_volume == -0.5));
}
