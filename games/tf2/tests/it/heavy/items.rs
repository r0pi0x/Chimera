//! The item converter on the user's install (specs/tf2/item_attributes.md
//! 5, its measured numbers): the schema read through the install's mount,
//! the item classes' scripts decrypted with the user's key when it is set
//! (never printed), the pack made in memory (`items::generated`), then
//! every batch-1 item of data/items.toml found with its attributes'
//! effective numbers as measured in the game, and the stock weapons with
//! every item weapon built over them. Without the install it passes with a
//! note; without the key the scripts' base numbers are left out (noted).

use tf2::game::{
    items::{self, ItemPack, ItemsData},
    mount, scripts,
    weapons::script_numbers::GunNumbers,
};

use super::maps::install;

#[test]
fn the_install_converts_into_an_item_pack_with_the_spec_numbers() {
    let Some(install) = install() else { return };
    let key = scripts::key().expect("the key's config");
    if key.is_none() {
        eprintln!("note: no .ctx key ({}): the item classes' scripts aren't read", scripts::KEY_ENV);
    }
    let plain = match mount::open_install(&install) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let source = scripts::decrypting(plain, key.clone());
    let sets = items::generated(&source, items::SET);
    assert_eq!(sets.len(), 1, "one set");
    let (set, files) = &sets[0];
    assert_eq!(set, items::SET);
    assert_eq!(files[0].0, items::PACK_PATH);
    let pack = ItemPack::parse(std::str::from_utf8(&files[0].1).unwrap()).unwrap();
    // Deterministic: a second conversion gives the same bytes.
    assert_eq!(items::generated(&source, items::SET)[0].1[0].1, files[0].1);
    assert!(pack.items.len() > 100, "{} weapon items", pack.items.len());

    let data = ItemsData::builtin();
    for (name, def) in &data.items {
        let r = pack.get(def.index).unwrap_or_else(|| panic!("{name} ({}) not in the pack", def.index));
        assert!(def.wearable || data.bases.contains_key(&r.class), "{name}: no base for {}", r.class);
    }
    let r = |name: &str| pack.get(data.items[name].index).unwrap();
    let a = |name: &str, class: &str| r(name).attr(class, 1.0);
    let close = |x: f64, y: f64| (x - y).abs() < 1e-3;
    // The attributes, folded (§5).
    assert!(close(a("winger", "mult_clipsize"), 0.4));
    assert!(close(a("winger", "mod_jump_height_from_weapon"), 1.25));
    assert!(close(r("rocket_jumper").attr("mult_dmg", 1.0), 0.0));
    assert!(close(a("brass_beast", "mult_player_aiming_movespeed"), 0.4));
    assert!(close(a("scotsmans_skullcutter", "mult_player_movespeed"), 0.85));
    assert!(r("scotsmans_skullcutter").flag("is_a_sword"));
    assert!(close(a("warriors_spirit", "mult_dmgtaken"), 1.3));
    assert!(close(r("bread_bite").attr("mod_maxhealth_drain_rate", 0.0), 10.0));
    // The effective numbers measured in the game (4.1-4.6).
    let e = |name: &str, k: &str| *r(name).effective.get(k).unwrap_or_else(|| panic!("{name} {k}: {:?}", r(name).effective));
    assert!(close(e("liberty_launcher", "projectile_speed"), 1540.0));
    assert!(close(e("direct_hit", "projectile_speed"), 1980.0));
    assert!(close(e("direct_hit", "blast_radius"), 43.8));
    assert!(close(e("iron_bomber", "fuse"), 1.4));
    assert!(close(e("quickiebomb_launcher", "charge"), 1.2));
    assert!(close(e("quickiebomb_launcher", "arm"), 0.6));
    assert!(close(e("sticky_jumper", "max_stickies"), 2.0));
    assert!(close(e("brass_beast", "spinup"), 1.125));
    assert!(close(e("tomislav", "spinup"), 0.6));
    assert!(close(e("brass_beast", "spun_speed"), 44.0));
    assert!(close(e("winger", "jump"), 361.25));
    if key.is_some() {
        for (name, clip) in [("winger", 5.0), ("pretty_boys_pocket_pistol", 9.0), ("liberty_launcher", 5.0), ("loch_n_load", 3.0), ("quickiebomb_launcher", 4.0), ("family_business", 8.0)] {
            assert_eq!(e(name, "clip"), clip, "{name}");
        }
    }

    // Every item weapon builds over the stock documents and parses.
    let numbers = GunNumbers::builtin().with_pack(&pack);
    let weapons = data.items.values().filter(|d| !d.wearable).count();
    assert_eq!(numbers.items.len(), weapons, "{:?}", numbers.sources.iter().filter(|s| s.gun == "items").collect::<Vec<_>>());
    numbers.parse().unwrap();
}

/// Each batch-1 item's looks on the install: its own `c_` model there,
/// its view model built on its class's arms (the arms' bones and the
/// model's merged), its sounds in the install's scripts.
#[test]
fn every_batch_one_item_shows_its_own_model_on_its_class_arms() {
    use source_engine::content::material::MaterialLoader;
    let Some(m) = super::maps::install_mount() else { return };
    let pack = ItemPack::parse(std::str::from_utf8(&items::generated(&m, items::SET)[0].1[0].1).unwrap()).unwrap();
    let data = ItemsData::builtin();
    let (all, notes) = items::weapons(&pack, &data, &GunNumbers::builtin().docs);
    assert!(notes.is_empty(), "{notes:?}");
    let batch: Vec<&items::ItemWeapon> =
        all.iter().filter(|i| data.items.values().any(|d| d.index == i.index && d.batch == 1)).collect();
    assert!(batch.len() >= 30, "{} batch-1 weapons", batch.len());
    let mut materials = MaterialLoader::guest(&m, 0, 0);
    let (mut views, mut warnings) = (Vec::new(), Vec::new());
    tf2::game::view::load_with(&mut materials, &|g| batch.iter().any(|i| i.id == g), false, &all, &mut views, &mut warnings);
    let sounds = tf2::game::sets::sound_scripts(&m);
    let mut own = 0;
    for item in &batch {
        let view = item.looks.view.as_deref().unwrap_or_else(|| panic!("{}: no model", item.id));
        assert!(m.contains(view), "{}: {view} not in the install", item.id);
        assert!(item.looks.world.as_deref().is_some_and(|w| m.contains(w)), "{}: {:?}", item.id, item.looks.world);
        let base = tf2::game::weapons::tf2_weapons().get(&item.base).and_then(|b| b.models.view.clone());
        own += usize::from(base.as_deref() != Some(view));
        for s in &item.looks.sounds {
            assert!(!sounds.waves(s).is_empty(), "{}: {s} has no waves", item.id);
        }
        // Worn or carried (a shield, boots, a watch): no class's arms hold
        // it, so no view model (`view::load_with` skips it).
        if item.looks.arms.is_empty() {
            assert!(!views.iter().any(|v| v.key == item.id), "{}: a view model with no arms", item.id);
            continue;
        }
        let v = views.iter().find(|v| v.key == item.id).unwrap_or_else(|| panic!("{}: no view model ({warnings:?})", item.id));
        let arms = source_engine::content::props::load_view_model(&mut materials, &tf2::game::sets::arms_model(&item.looks.arms), "arms", true).unwrap();
        assert!(v.bones.len() >= arms.bones.len(), "{}: {} bones, the arms {}", item.id, v.bones.len(), arms.bones.len());
        assert!(v.bones.iter().any(|b| b.name.eq_ignore_ascii_case("weapon_bone")), "{}", item.id);
        assert!(v.model.meshes.len() > arms.model.meshes.len(), "{}: the weapon's meshes merged", item.id);
        assert!(v.animations.is_some(), "{}: the arms' sequences", item.id);
        assert!(tf2::game::view::sequences(&item.base).is_some() || tf2::game::view::melee_sequences(&item.base).is_some(), "{}", item.id);
    }
    assert!(own >= 25, "{own} of {} items have a model of their own", batch.len());
}

/// An item's asset set (`sets::item_pieces`, what `chimera_pack sets tf2`
/// publishes as `weapon/<name>`) packed into a scratch store under
/// `target/` with what a match offers with it (`players`, its class's
/// `players/<class>`) and the item pack (`items::generated`), then read
/// back from that store alone, as a player served by the asset service
/// with no install: the item registers (`items::on_mount`), its view
/// model is its own `c_` model on its class's arms with nothing missing,
/// and its set holds neither the arms nor its class's sequences (they
/// are the class set's) nor its base's set's files, but its own model
/// and sounds. A map's dressing
/// takes only the unlocks whose sets are here, and one loaded late
/// (`late_items::load`) builds from the store, or waits for its set.
#[test]
fn an_items_set_serves_its_own_view_model_from_the_store_alone() {
    use std::path::Path;

    use chimera_core::mount::{
        assets::{AssetSettings, FileRecord, Manifest, Store, pack},
        served,
    };
    use source_engine::content::material::MaterialLoader;
    use tf2::game::{GAME, classes, sets};

    let Some(install) = install() else { return };
    if !install.join("tf").is_dir() {
        eprintln!("skipping: packs from the install's own files (served from the asset store here)");
        return;
    }
    let key = scripts::key().expect("the key's config");
    let source = scripts::decrypting(mount::open_install(&install).unwrap(), key);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tf2-item-sets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = Store::new(&root);
    let build = "item-sets-test";
    let mut manifest = Manifest::new(GAME, build, Some(440));

    let sounds = sets::sound_scripts(&source);
    let players = sets::player_pieces(&source);
    let base = sets::M2_WEAPONS.iter().find(|w| w.id == "tf_weapon_rocketlauncher").unwrap();
    let base_files = sets::weapon_files(&source, base, &sounds);
    let mut shared = players.clone();
    shared.push(("weapon/tf_weapon_rocketlauncher".into(), base_files.clone()));
    let pieces = sets::item_pieces(&source, &shared, &sounds);
    let all = sets::item_weapons(&source);
    assert_eq!(pieces.len(), all.len(), "a set per item weapon");
    let id = "tf2:direct_hit";
    let item = all.iter().find(|i| i.id == id).expect("the Direct Hit");
    let set = sets::item_set(item);
    assert_eq!(set, "weapon/direct_hit");
    assert!(sets::is_item_set(&set) && !sets::is_item_set("weapon/tf_weapon_rocketlauncher"));
    let files = &pieces.iter().find(|(n, _)| *n == set).expect("its set").1;
    let view = item.looks.view.clone().expect("its own model");
    assert!(files.contains(&view), "{set}: {view}");
    // Its own sounds' waves: in its set, or in one that comes with it.
    let anywhere = |p: &String| files.contains(p) || shared.iter().any(|(_, f)| f.contains(p));
    for w in item.looks.sounds.iter().flat_map(|e| sounds.waves(e)).filter(|w| source.contains(w)) {
        assert!(anywhere(&w), "{set}: {w}");
    }
    assert!(!files.iter().any(|p| p.contains("_arms.") || p.ends_with("_animations.mdl")), "{set}: the arms are the class set's");
    assert!(!files.iter().any(|p| base_files.contains(p)), "{set}: nothing of its base's set");
    // A second making gives the same set.
    assert_eq!(sets::item_files(&source, item, &all, &sounds).iter().filter(|p| files.contains(p)).count(), files.len());

    let class = classes::set_of("soldier");
    for (name, paths) in players.iter().filter(|(n, _)| *n == "players" || *n == class).chain([(set.clone(), files.clone())].iter()) {
        let report = pack(&source, paths, &store, &mut manifest, Some(name), 2).unwrap();
        assert!(report.missing.is_empty(), "{name}: {:?}", report.missing);
        eprintln!("{name}: {} files, {} bytes stored", report.files, report.stored);
    }
    for (name, made) in items::generated(&source, items::SET) {
        for (path, bytes) in made {
            let put = store.put(&bytes).unwrap();
            manifest.files.insert(path.clone(), FileRecord { id: put.id, size: bytes.len() as u64, stored: put.stored });
            manifest.sets.entry(name.clone()).or_default().insert(path);
        }
    }
    manifest.save(&store.manifest_path(GAME, build)).unwrap();

    // The store alone.
    let settings = AssetSettings {
        store: Some(store.clone()),
        use_install: false,
        service_first: false,
        builds: [(GAME.to_string(), build.to_string())].into(),
        remote: None,
        lock: None,
    };
    let served = served::open_with(&settings, GAME, Path::new("no-install"), mount::open_install).unwrap();
    assert!(!served.contains("models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"), "the base's model isn't served");
    let on = items::on_mount(&served);
    let served_item = on.iter().find(|i| i.id == id).expect("the item registers from the store");
    assert_eq!(served_item.looks.view.as_deref(), Some(view.as_str()));
    let mut materials = MaterialLoader::guest(&served, 0, 0);
    let (mut views, mut warnings) = (Vec::new(), Vec::new());
    tf2::game::view::load_with(&mut materials, &|g| g == id, false, &on, &mut views, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    let v = views.iter().find(|v| v.key == id).unwrap_or_else(|| panic!("no view model: {warnings:?}"));
    let arms = source_engine::content::props::load_view_model(&mut materials, &sets::arms_model("soldier"), "arms", true).unwrap();
    assert!(v.model.meshes.len() > arms.model.meshes.len(), "its own model merged on the arms");
    assert!(v.animations.is_some(), "the arms' sequences");

    // A map's dressing takes only the unlocks whose sets are here
    // (`items::on_mount_here`): the Winger's isn't in this store.
    let here = items::on_mount_here(&served);
    assert!(here.iter().any(|i| i.id == id) && !here.iter().any(|i| i.id == "tf2:winger"), "{:?}", here.iter().map(|i| &i.id).collect::<Vec<_>>());
    // Loaded late (`late_items`, the sandbox's pick after the map): built
    // from the store when its set is here, waiting when not.
    let open = move || served::open_with(&settings, GAME, Path::new("no-install"), mount::open_install).map_err(|e| e.to_string());
    let files = tf2::game::late_bodies::Tf2Files(std::sync::Arc::new(open));
    use tf2::game::late_items::{Outcome, load};
    match load(&files, id, true).unwrap() {
        Outcome::Built(b) => {
            assert!(b.view_models.iter().any(|v| v.key == id), "its view model");
            assert!(b.held.iter().any(|h| h.key == id), "its held model");
            assert!(item.looks.sounds.iter().all(|e| b.sounds.entries.contains_key(&tf2::game::maps::own_sound(e).to_lowercase())), "its sound entries");
        }
        Outcome::NotHere => panic!("{id}: its set is here"),
    }
    assert!(matches!(load(&files, "tf2:winger", true).unwrap(), Outcome::NotHere), "the Winger's set isn't here");
    let _ = std::fs::remove_dir_all(&root);
}

/// The Heavy's glove unlocks in first person (the playtest's "gloves
/// rotated"): each plays its own sequences on the Heavy's arms (by
/// activity, `view::item_activity`: the Gloves of Running Urgently, the
/// Fists of Steel and the Apoco-Fists the `bg_*` set, the Bread Bite its
/// `breadglove_*`, the Warrior's Spirit and the Eviction Notice the Fists'
/// `f_*` by their replacement), and at frame 0 of its idle every bone of
/// its model (a joint of the merged view model, `map::merge_keeping`)
/// sits on the arms' bone it follows, and every bone the glove rigs on a
/// hand (`vm_weapon_bone…` under `bip_hand_R`/`_L`) is where the glove's
/// own model puts it from that hand, within 3 units and 25 degrees (the
/// idles move them a little; the stock Fists' `f_idle` leaves them 11
/// units off, under the weapons' place in the hand): the
/// glove on the hand, not off it or turned.
#[test]
fn the_heavy_gloves_sit_on_the_hands_at_their_idle() {
    use bevy::prelude::*;
    use chimera_world::map::{anim::Animator, posed_world, reference_world};
    use source_engine::content::material::MaterialLoader;
    let Some(m) = super::maps::install_mount() else { return };
    let pack = ItemPack::parse(std::str::from_utf8(&items::generated(&m, items::SET)[0].1[0].1).unwrap()).unwrap();
    let (all, _) = items::weapons(&pack, &ItemsData::builtin(), &GunNumbers::builtin().docs);
    let gloves = [
        ("warriors_spirit", "f_idle"),
        ("fists_of_steel", "bg_idle"),
        ("apoco_fists", "bg_idle"),
        ("bread_bite", "breadglove_idle"),
        ("gloves_of_running_urgently", "bg_idle"),
        ("eviction_notice", "f_idle"),
    ];
    let ids: Vec<String> = gloves.iter().map(|(g, _)| format!("tf2:{g}")).collect();
    let mut materials = MaterialLoader::guest(&m, 0, 0);
    let (mut views, mut warnings) = (Vec::new(), Vec::new());
    tf2::game::view::load_with(&mut materials, &|g| ids.iter().any(|i| i == g), false, &all, &mut views, &mut warnings);
    for ((glove, idle), id) in gloves.iter().zip(&ids) {
        let item = all.iter().find(|i| &i.id == id).unwrap_or_else(|| panic!("{id}: not in the pack"));
        let v = views.iter().find(|v| &v.key == id).unwrap_or_else(|| panic!("{id}: no view model ({warnings:?})"));
        let doc: toml::Table = item.doc.parse().unwrap();
        let hooks = doc.get("hooks").and_then(toml::Value::as_table).unwrap();
        let pick = tf2::game::view::item_activity(hooks, "ACT_VM_IDLE").unwrap_or_else(|| panic!("{id}: plays by activity"));
        let set = v.animations.clone().unwrap();
        let s = set.sequences.iter().position(|q| pick.matches(q)).unwrap_or_else(|| panic!("{id}: no sequence for {pick:?}"));
        assert!(set.sequences[s].name.starts_with(idle), "{id}: idles in {}", set.sequences[s].name);
        let mut a = Animator::new(set.clone());
        a.play(s, 0.0);
        let pose = a.pose(0.0);
        assert_eq!(pose.len(), v.bones.len(), "{id}: every joint posed");
        let posed = posed_world(&v.bones, &pose);
        // The glove's own model, as its reference pose has it.
        let model = item.looks.view.as_deref().unwrap();
        let own = source_engine::content::props::load_view_model(&mut materials, model, glove, true).unwrap();
        let own_ref = reference_world(&own.bones);
        let named = |n: &str| v.bones.iter().position(|b| b.name.eq_ignore_ascii_case(n));
        let mut checked = 0;
        for (k, b) in own.bones.iter().enumerate() {
            let Some(arms_bone) = named(&b.name) else { continue };
            // Its joint, on the arms' bone.
            let j = named(&format!("{}#merged", b.name)).unwrap_or_else(|| panic!("{id}: {} not merged", b.name));
            assert_eq!(v.bones[j].parent, Some(arms_bone), "{id}: {}", b.name);
            let d = posed[j].w_axis.truncate().distance(posed[arms_bone].w_axis.truncate());
            let (_, rj, _) = posed[j].to_scale_rotation_translation();
            let (_, ra, _) = posed[arms_bone].to_scale_rotation_translation();
            assert!(d < 1e-3 && rj.angle_between(ra) < 1e-3, "{id}: {}'s joint {d} units, {} rad off its arms bone", b.name, rj.angle_between(ra));
            // A bone rigged on a hand: where the glove puts it from the hand.
            let Some(p) = b.parent.filter(|p| own.bones[*p].name.to_ascii_lowercase().starts_with("bip_hand")) else { continue };
            let hand = named(&own.bones[p].name).unwrap();
            let want = own_ref[p].inverse() * own_ref[k];
            let got = posed[hand].inverse() * posed[arms_bone];
            let (_, wr, wt) = want.to_scale_rotation_translation();
            let (_, gr, gt) = got.to_scale_rotation_translation();
            let (off, turn) = (wt.distance(gt), wr.angle_between(gr).to_degrees());
            eprintln!("{id}: {} from {}: {off:.2} units, {turn:.1} degrees", b.name, own.bones[p].name);
            assert!(off < 3.0 && turn < 25.0, "{id}: {} is {off} units and {turn} degrees from where the glove rigs it on {}", b.name, own.bones[p].name);
            checked += 1;
        }
        eprintln!("{id}: {} bones, {checked} on a hand", own.bones.len());
    }
}
