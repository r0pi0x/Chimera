//! The weapon scripts' numbers over the data (specs/tf2/weapon_scripts.md,
//! "Test cases"): synthetic scripts made up here (invented numbers, never
//! the game's) mapped onto the guns' documents (`script_numbers`), and a
//! headless match (`Tf2Sim`) whose guns registered from them: the given
//! guns carry the script's clip, pellets, interval, range and view kick
//! (the corrections from the SDK), the rocket's blast is the script's, the
//! flame's damage and a shell gun's insert are not (the game ignores the
//! one and times the other by its animation; only the Grenade Launcher's
//! insert is the script's), and each gun says where its numbers came
//! from.

use chimera_sim::weapon::{Hitscan, Magazine, Trigger, data::WeaponCatalog, give};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use source_formats::keyvalues::{Kv, parse_pc};
use tf2::{
    game::{
        TICK_INTERVAL,
        scripts::{Script, ScriptSource},
        weapons::{
            FLAMETHROWER, GRENADE_LAUNCHER, ROCKET_LAUNCHER, SCATTERGUN, SNIPER_RIFLE, SHOTGUN,
            rocket::RocketRules,
            script_numbers::{GunNumbers, apply_script, gun_doc},
        },
    },
    harness::Tf2Sim,
};
use toml::Table;

fn plain(name: &str, text: &str) -> Script {
    Script { name: name.into(), source: ScriptSource::Plain, root: parse_pc(text), note: None }
}

fn missing(name: &str) -> Script {
    Script { name: name.into(), source: ScriptSource::Missing, root: Kv::Block(Vec::new()), note: Some("not in this test".into()) }
}

/// Invented scripts for four guns; every other gun has none.
fn made_up(name: &str) -> Script {
    match name {
        "tf_weapon_scattergun" => plain(
            name,
            r#""WeaponData" { "WeaponType" "primary" "Damage" "7" "BulletsPerShot" "3" "Spread" "0.05"
               "TimeFireDelay" "0.3" "clip_size" "5" "default_clip" "5" "TimeReload" "0.4" "TimeReloadStart" "0.01"
               "Range" "1000" "PunchAngle" "2" "ProjectileType" "projectile_bullet" }"#,
        ),
        // Another root name, lower-case keys.
        "tf_weapon_rocketlauncher" => plain(
            name,
            r#""SomethingElse" { "damage" "60" "DamageRadius" "120" "ProjectileType" "projectile_rocket" "clip_size" "3" }"#,
        ),
        "tf_weapon_grenadelauncher" => plain(
            name,
            r#""WeaponData" { "damage" "60" "DamageRadius" "120" "ProjectileType" "projectile_pipe" }"#,
        ),
        "tf_weapon_flamethrower" => plain(name, r#""WeaponData" { "Damage" "200" "TimeFireDelay" "0.03" }"#),
        "tf_weapon_sniperrifle" => plain(name, r#""WeaponData" { "Damage" "3" "TimeFireDelay" "1.1" }"#),
        // The kind check: a rocket's script on a shotgun.
        "tf_weapon_shotgun_primary" => plain(name, r#""WeaponData" { "Damage" "99" "ProjectileType" "projectile_rocket" }"#),
        _ => missing(name),
    }
}

struct WithNumbers(GunNumbers);

impl bevy::app::Plugin for WithNumbers {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(self.0.clone());
        bevy::app::Plugin::build(&Tf2Sim, app);
    }
}

#[test]
fn a_match_plays_with_the_scripts_numbers() {
    let numbers = GunNumbers::with_scripts(made_up);
    let source = |gun: &str| numbers.sources.iter().find(|s| s.gun == gun).unwrap().clone();
    assert!(source(SCATTERGUN).from_script, "{:?}", source(SCATTERGUN));
    assert!(source(ROCKET_LAUNCHER).from_script);
    assert!(source(SNIPER_RIFLE).from_script, "its interval");
    assert!(!source(SHOTGUN).from_script && source(SHOTGUN).source.contains("ProjectileType"), "{:?}", source(SHOTGUN));
    assert!(source(tf2::game::weapons::MINIGUN).source.starts_with("spec fallback"));
    assert_eq!(numbers.sources.len(), 21, "twelve guns and nine melee weapons");

    let mut sim = Sim::new(WithNumbers(numbers), GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    let catalog = sim.app.world().resource::<WeaponCatalog>().clone();
    let scatter = catalog.get(SCATTERGUN).unwrap();
    assert_eq!(scatter.damage.as_ref().unwrap().amount, 7.0);
    assert_eq!(scatter.hitscan.as_ref().unwrap().pellets, 3);
    assert_eq!(scatter.hitscan.as_ref().unwrap().range, 1000.0, "Range is the bullets' trace");
    assert_eq!(scatter.hooks["tf2:gun"]["punch"].as_float(), Some(2.0), "PunchAngle is the view kick");
    let mag = scatter.magazine.as_ref().unwrap();
    assert_eq!((mag.clip, mag.reload), (5, 0.5), "TimeReload isn't a Scattergun's insert");
    let shells = mag.shells.as_ref().unwrap();
    assert_eq!((shells.start, shells.insert), (0.2, 0.5), "the start and insert stay the data's");
    // The rest of the base's tables stay (inherited, merged).
    assert_eq!(scatter.damage.as_ref().unwrap().falloff_step, 1024.0);
    let sniper = catalog.get(SNIPER_RIFLE).unwrap();
    assert_eq!(sniper.trigger.as_ref().unwrap().cycle, 1.1);
    assert_ne!(sniper.damage.as_ref().unwrap().amount, 3.0, "the rifle's Damage isn't its damage");
    let flame = catalog.get(FLAMETHROWER).unwrap();
    let data_flame = tf2::game::weapons::tf2_weapons();
    let data_flame = data_flame.get(FLAMETHROWER).unwrap();
    assert_eq!(flame.hooks["tf2:flame"]["damage"], data_flame.hooks["tf2:flame"]["damage"], "the flame's Damage is ignored");
    assert_eq!(flame.trigger.as_ref().unwrap().cycle, 0.02, "its TimeFireDelay isn't the cycle (a flame every 0.02 s)");
    let grenade = catalog.get(GRENADE_LAUNCHER).unwrap();
    assert_eq!(grenade.hooks["tf2:grenade"]["radius"].as_float(), Some(120.0));
    assert_eq!(catalog.get(SHOTGUN).unwrap().damage.as_ref().unwrap().amount, 6.0, "refused: the data's");

    // The rocket's blast rules read the registered set: the script's
    // damage, the data's radius (the rocket's script has none to map).
    let rules = sim.app.world().resource::<RocketRules>();
    let (rocket, _) = rules.get(ROCKET_LAUNCHER).unwrap();
    assert_eq!((rocket.damage, rocket.radius), (60.0, 146.0));

    // A player given the guns holds the script's numbers.
    let p = sim.spawn_character(bevy::math::Vec3::new(0.0, 1.0, 0.0), tf2::game::movement::ID);
    sim.seconds(0.5);
    let w = give(sim.app.world_mut(), p, SCATTERGUN).unwrap();
    let world = sim.app.world();
    assert_eq!(world.get::<Magazine>(w).unwrap().clip, 5);
    assert_eq!(world.get::<Hitscan>(w).unwrap().pellets, 3);
    assert!((world.get::<Trigger>(w).unwrap().cycle - 0.3).abs() < 1e-6);
    let r = give(sim.app.world_mut(), p, ROCKET_LAUNCHER).unwrap();
    assert_eq!(sim.app.world().get::<Magazine>(r).unwrap().clip, 3);
}

/// The built-in numbers are the committed documents as they are.
#[test]
fn the_harness_plays_with_the_committed_numbers() {
    let sim = Sim::new(Tf2Sim, GreyboxMapPlugin);
    let numbers = sim.app.world().resource::<GunNumbers>();
    assert!(numbers.is_builtin());
    let catalog = sim.app.world().resource::<WeaponCatalog>();
    assert_eq!(catalog.get(SCATTERGUN).unwrap().damage.as_ref().unwrap().amount, 6.0);
}

fn doc(text: &str) -> Table {
    text.parse().unwrap()
}

const BULLET: &str = "id = \"tf2:x\"\nslot = 0\n[trigger]\ncycle = 0.3\n[magazine]\nclip = 9\nreload = 1.2\n[hitscan]\npellets = 1\nspread = { template = 0.01 }\n[damage]\namount = 5.0\n";

#[test]
fn the_spec_rows_on_a_bullet_gun() {
    // Only Damage: the rest stays.
    let mut d = doc(BULLET);
    let a = apply_script(plain("x", r#""W" { "Damage" "7" }"#).data().unwrap(), &mut d);
    assert_eq!(a.set, ["[damage] amount"]);
    assert_eq!((d["trigger"]["cycle"].as_float(), d["magazine"]["clip"].as_integer()), (Some(0.3), Some(9)));
    // clip -1: no clip, the data's.
    let mut d = doc(BULLET);
    apply_script(plain("x", r#""W" { "clip_size" "-1" }"#).data().unwrap(), &mut d);
    assert_eq!(d["magazine"]["clip"].as_integer(), Some(9));
    // A whole-clip gun's TimeReload isn't its reload.
    let mut d = doc(BULLET);
    apply_script(plain("x", r#""W" { "TimeReload" "0.3" }"#).data().unwrap(), &mut d);
    assert_eq!(d["magazine"]["reload"].as_float(), Some(1.2));
    // A fractional Damage is cut toward zero, as the game reads it.
    let mut d = doc(BULLET);
    apply_script(plain("x", r#""W" { "Damage" "7.5" }"#).data().unwrap(), &mut d);
    assert_eq!(d["damage"]["amount"].as_float(), Some(7.0));
    // Range: the bullets' trace.
    let mut d = doc(BULLET);
    apply_script(plain("x", r#""W" { "Range" "1000" }"#).data().unwrap(), &mut d);
    assert_eq!(d["hitscan"]["range"].as_float(), Some(1000.0));
    // A secondary's script on a primary: nothing.
    let mut d = doc(BULLET);
    let a = apply_script(plain("x", r#""W" { "Damage" "7" "WeaponType" "secondary" }"#).data().unwrap(), &mut d);
    assert!(a.refused.is_some() && a.set.is_empty());
    assert_eq!(d["damage"]["amount"].as_float(), Some(5.0));
    // AmmoPerShot and default_clip are noted, not applied.
    let mut d = doc(BULLET);
    let a = apply_script(plain("x", r#""W" { "clip_size" "8" "default_clip" "6" "AmmoPerShot" "2" }"#).data().unwrap(), &mut d);
    assert_eq!(d["magazine"]["clip"].as_integer(), Some(8));
    assert_eq!(a.notes.len(), 2, "{:?}", a.notes);
}

#[test]
fn a_needle_gun_takes_the_damage_as_its_needles() {
    let mut d = doc("id = \"tf2:n\"\nslot = 0\n[hooks.\"tf2:needle\"]\ndamage = 10.0\n");
    let a = apply_script(plain("x", r#""W" { "Damage" "12" "ProjectileType" "projectile_syringe" }"#).data().unwrap(), &mut d);
    assert_eq!(a.set, ["tf2:needle damage"]);
    assert_eq!(d["hooks"]["tf2:needle"]["damage"].as_float(), Some(12.0));
}

#[test]
fn a_gun_without_a_script_keeps_its_text() {
    let (text, s) = gun_doc("tf2:x", BULLET, &missing("x"));
    assert_eq!(text, BULLET);
    assert!(s.source.starts_with("spec fallback"));
}

/// A dedicated server with neither TF2's install nor the scripts' key:
/// with a credential of its own it fetches TF2's `base` from the asset
/// service before it starts (`chimera_net::net::server_assets`), the
/// scripts in it stored decrypted (here: invented plain scripts), and its
/// guns' numbers and so its `tf2:weapons` server documents come from
/// them, read from its store with nothing left to fetch, not the specs'
/// fallback.
#[test]
fn a_server_without_install_or_key_takes_its_numbers_from_the_asset_service() {
    use std::collections::BTreeSet;

    use chimera_core::mount::{
        FileSource, Mount,
        assets::{AssetSettings, CachedFiles, FileRecord, Manifest, RemoteConfig, Store, accounts::Accounts, server},
    };
    use chimera_net::net::server_assets::{self, ServerFiles, Via};
    use chimera_sim::server_docs::ServerDocs;
    use tf2::game::weapons::SERVER_DOCS;

    let dir = std::env::temp_dir().join(format!("tf2_server_files_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // The host's published store: TF2's `base` with the scattergun's and
    // the rocket launcher's scripts (plain, as `chimera_pack` packs them).
    let published = Store::new(dir.join("published"));
    let mut m = Manifest::new("tf2", "1", None);
    for name in ["tf_weapon_scattergun", "tf_weapon_rocketlauncher"] {
        let text = match name {
            "tf_weapon_scattergun" => r#""WeaponData" { "WeaponType" "primary" "Damage" "7" "BulletsPerShot" "3" "clip_size" "5" }"#,
            _ => r#""WeaponData" { "damage" "60" "ProjectileType" "projectile_rocket" "clip_size" "3" }"#,
        };
        let path = format!("scripts/{name}.ctx");
        let p = published.put(text.as_bytes()).unwrap();
        m.files.insert(path.clone(), FileRecord { id: p.id, size: text.len() as u64, stored: p.stored });
        m.sets.entry("base".into()).or_default().insert(path);
    }
    m.save(&published.manifest_path("tf2", "1")).unwrap();
    let accounts = Accounts::path(&published.root);
    let mut a = Accounts::load(&accounts).unwrap();
    let (_, token) = a.add("game-server", None, "test").unwrap();
    a.save(&accounts).unwrap();
    let running = server::serve(server::Service::new(published.clone()).unwrap(), "127.0.0.1:0", 2).unwrap();
    // The server: an empty store of its own, the service on this machine
    // with its token.
    let own = Store::new(dir.join("server"));
    let files = ServerFiles {
        settings: AssetSettings {
            store: Some(own.clone()),
            use_install: false,
            service_first: false,
            builds: Default::default(),
            remote: Some(RemoteConfig { url: format!("http://{}", running.addr), token, key: None, fingerprint: None }),
            lock: None,
        },
        served: BTreeSet::from(["tf2".to_string()]),
        via: Via::Config,
    };
    let files = server_assets::fetch_at_start(files, "greybox");
    assert!(own.manifest_path("tf2", "1").is_file(), "the manifest fetched");
    let mut mount = Mount::default();
    let cached = CachedFiles::open(&own, "tf2", "1").unwrap();
    assert!(cached.read("scripts/tf_weapon_scattergun.ctx").is_some_and(|r| r.is_ok()), "in the store");
    mount.push(cached);
    // No key: the stored scripts are plain.
    let numbers = GunNumbers::from_mount(Some(&mount), None);
    let source = |gun: &str| numbers.sources.iter().find(|s| s.gun == gun).unwrap().clone();
    assert!(source(SCATTERGUN).from_script, "{:?}", source(SCATTERGUN));
    assert!(source(ROCKET_LAUNCHER).from_script, "{:?}", source(ROCKET_LAUNCHER));
    assert!(source(SHOTGUN).source.starts_with("spec fallback"), "a gun with no script stays the spec's");
    let sim = Sim::new(WithNumbers(numbers), GreyboxMapPlugin);
    let catalog = sim.app.world().resource::<WeaponCatalog>();
    let scatter = catalog.get(SCATTERGUN).unwrap();
    assert_eq!((scatter.damage.as_ref().unwrap().amount, scatter.hitscan.as_ref().unwrap().pellets), (7.0, 3));
    let docs = sim.app.world().resource::<ServerDocs>().get(SERVER_DOCS).unwrap();
    let scatter_doc = docs.docs.iter().find(|(_, t)| t.contains(&format!("\"{SCATTERGUN}\""))).unwrap();
    assert!(scatter_doc.1.contains("amount = 7"), "the server documents carry the script's numbers: {}", scatter_doc.1);
    drop(files);
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A melee script onto a melee weapon's document (weapons_stock.md 6;
/// cases 33-35): `Damage`, `TimeFireDelay` as the hit and miss refires,
/// `SmackDelay` as the delay, 0.2 s without one; a melee script on a gun
/// applies nothing.
#[test]
fn melee_scripts_map_33_34_35() {
    let melee = "slot = 2\n[melee.primary]\ndamage = 65.0\nhit_refire = 0.8\nmiss_refire = 0.8\nmiss_refire_other = 0.8\ndelay = 0.2\n";
    let num = |t: &Table, k: &str| t["melee"]["primary"][k].as_float().or(t["melee"]["primary"][k].as_integer().map(|i| i as f64));
    // 33.
    let mut doc: Table = melee.parse().unwrap();
    let s = plain("x", r#""WeaponData" { "Damage" "50" "TimeFireDelay" "0.6" "SmackDelay" "0.15" "MeleeWeapon" "1" "WeaponType" "melee" }"#);
    let a = apply_script(s.data().unwrap(), &mut doc);
    assert!(a.refused.is_none(), "{a:?}");
    assert_eq!(num(&doc, "damage"), Some(50.0));
    for k in ["hit_refire", "miss_refire", "miss_refire_other"] {
        assert_eq!(num(&doc, k), Some(0.6), "{k}");
    }
    assert_eq!(num(&doc, "delay"), Some(0.15));
    // 34.
    let mut doc: Table = melee.replace("delay = 0.2", "delay = 0.1").parse().unwrap();
    let s = plain("x", r#""WeaponData" { "Damage" "50" "MeleeWeapon" "1" }"#);
    apply_script(s.data().unwrap(), &mut doc);
    assert_eq!(num(&doc, "delay"), Some(0.2));
    // 35.
    let mut gun: Table = "slot = 0\n[damage]\namount = 6.0\n".parse().unwrap();
    let s = plain("x", r#""WeaponData" { "Damage" "50" "MeleeWeapon" "1" }"#);
    let a = apply_script(s.data().unwrap(), &mut gun);
    assert!(a.refused.is_some() && a.set.is_empty(), "{a:?}");
    assert_eq!(gun["damage"]["amount"].as_float(), Some(6.0));
}

/// weapon_scripts.md's corrections from the SDK: `TimeReload` is the
/// insert of the Grenade Launcher only; a shotgun keeps its animation's.
#[test]
fn only_the_grenade_launcher_takes_time_reload() {
    const SHELLS: &str = "slot = 0\n[magazine]\nclip = 4\nreload = 0.4\nshells = { start = 0.25, insert = 0.4 }\n";
    let grenade = format!("{SHELLS}[hooks.\"tf2:grenade\"]\ndamage = 50.0\n");
    let mut d = doc(&grenade);
    apply_script(plain("x", r#""W" { "TimeReload" "0.3" "TimeReloadStart" "0.05" "ProjectileType" "projectile_pipe" }"#).data().unwrap(), &mut d);
    assert_eq!(d["magazine"]["reload"].as_float(), Some(0.3));
    assert_eq!(d["magazine"]["shells"]["insert"].as_float(), Some(0.3));
    assert_eq!(d["magazine"]["shells"]["start"].as_float(), Some(0.25), "the start stays");
    let shotgun = format!("{SHELLS}[damage]\namount = 6.0\n");
    let mut d = doc(&shotgun);
    apply_script(plain("x", r#""W" { "TimeReload" "0.3" }"#).data().unwrap(), &mut d);
    assert_eq!(d["magazine"]["shells"]["insert"].as_float(), Some(0.4), "a shotgun's insert is its animation's");
}

/// The flame's `Damage` and `TimeFireDelay` are ignored (13 a flame,
/// every 0.02 s, whatever the script says).
#[test]
fn the_flames_damage_and_interval_are_ignored() {
    let mut d = doc("slot = 0\n[trigger]\ncycle = 0.08\n[hooks.\"tf2:flame\"]\ndamage = 13.0\n");
    let a = apply_script(plain("x", r#""W" { "Damage" "150" "TimeFireDelay" "0.03" }"#).data().unwrap(), &mut d);
    assert!(a.set.is_empty(), "{:?}", a.set);
    assert_eq!(d["hooks"]["tf2:flame"]["damage"].as_float(), Some(13.0));
    assert_eq!(d["trigger"]["cycle"].as_float(), Some(0.08));
}
