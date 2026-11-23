//! The Elite's units from the disc (`game::campaign::unit`,
//! specs/halo_ce/characters.md sections 3, 5 and 8): cut from d20, the
//! body and the voice together are the spec's closure (161 tags, 6 682 822
//! bytes); the body's file loads as a map whose Elite becomes a character
//! model (its height, a head hit box among its boxes, its graph's plasma
//! rifle and needler stances, its deaths) with no voice in it; its tables
//! are the spec's numbers; its guns are held on its own hand's bone; its
//! ankles stand where the bind pose has them (feet on the origin) and a
//! walking foot plants there, as the Spartan's do. Prints the measured
//! numbers. Passes with a note without an install.

use blam_formats::cache::MapFile;
use halo_ce::game::{
    METERS_PER_UNIT,
    body::{anims::key, load},
    campaign::{BodyKind, BodyTables, CampaignData, unit},
    maps,
    movement::{self, MovementData},
};

use super::maps::mount;

#[test]
fn the_elite_is_cut_from_d20_and_loads() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let def = data.body(BodyKind::Elite).unwrap();
    let bytes = mount.read(&maps::map_path(&def.donor_map)).unwrap();
    let c = unit::convert("elite", def, &bytes).unwrap();
    assert_eq!(c.body_unit.tags.len() + c.voice_unit.tags.len(), 161);
    assert_eq!(c.body_unit.total() + c.voice_unit.total(), 6_682_822);
    assert_eq!(c.body, unit::convert("elite", def, &bytes).unwrap().body, "deterministic");
    let file = MapFile::load(&c.body).unwrap();
    let body = load::biped_body(&file, &def.biped, false, def.tint(&c.ranks)).unwrap();
    let m = &body.character;
    // Its armour takes the player's colour through slot A (every armour
    // shader), and wears its rank's (the minor's blue, then the teams':
    // major's dark red, minor's), read from d20's actor variants.
    let skins = &m.model.skins;
    assert_eq!(skins.len(), halo_ce::game::body::COLOURS.len() + 3 * halo_ce::game::body::RANK_STEPS);
    assert_eq!(c.ranks.ranks.keys().map(String::as_str).collect::<Vec<_>>(), ["major", "minor", "specops"]);
    assert_eq!(c.ranks.ranks["major"].permutation, 1);
    assert_eq!(c.ranks.to_text(), unit::convert("elite", def, &bytes).unwrap().ranks.to_text(), "deterministic");
    let tinted = skins[0].iter().filter(|l| l.tint.is_some()).count();
    assert!(tinted * 2 > skins[0].len(), "{tinted} of {} meshes tinted", skins[0].len());
    let blue = c.ranks.ranks["minor"].lower;
    assert!((blue[0] - 0.314).abs() < 1e-3 && (blue[2] - 0.533).abs() < 1e-3, "{blue:?}");
    let near = |t: Option<[f32; 3]>| t.is_some_and(|t| (0..3).all(|i| (t[i] - blue[i]).abs() < 1e-3));
    assert!(skins.last().unwrap().iter().any(|l| near(l.tint)));
    let set = m.animations.as_ref().unwrap();
    for name in ["stand pr idle", "stand pr move-front", "stand ne idle", "crouch pr idle", "stand pr aim-still"] {
        assert!(set.names.contains_key(name), "{name}");
    }
    assert!(set.names.contains_key("damage s-kill front gut"));
    assert!(set.names.contains_key("damage h-kill back gut"));
    assert!(!set.names.contains_key(&key("stand", "ar", "idle")), "no standing assault rifle");
    assert!(load::death_lines(&file, &def.biped).iter().all(Option::is_none), "the voice is its own unit");
    let heads = m.hitboxes.iter().filter(|h| h.region == chimera_core::core::BodyRegion::Head).count();
    assert_eq!(heads, 1);
    // Its guns by its own hand marker (`right hand elite`), on its own bone.
    let held = unit::held_on(BodyKind::Elite, def, &file, &mount, &["halo_ce:plasma_rifle", "halo_ce:needler"], false).unwrap();
    assert_eq!(held.models.len(), 2);
    let pr = &held.models[0];
    assert!(pr.key.ends_with("#elite") && !pr.model.meshes.is_empty(), "{}", pr.key);
    assert!(m.bones.iter().any(|b| b.name == pr.bone), "the hand's bone `{}` is the Elite's", pr.bone);
    eprintln!("elite: plasma rifle held on `{}`", pr.bone);
    // Its sounds: its own footsteps (six kinds), its ten filled pain and
    // death slots (characters.md section 6: pain body minor, major,
    // falling, scream pain, six deaths); none without the voice set.
    let voice_file = MapFile::load(&c.voice).unwrap();
    let sounds = halo_ce::game::campaign::voice::compile(def, &file, Some(&voice_file));
    assert_eq!(sounds.set.library.footsteps.len(), 6);
    assert!(sounds.set.library.footsteps[1].iter().any(|s| !s.is_empty()), "run steps");
    assert_eq!(sounds.lines[..halo_ce::game::campaign::voice::SLOTS].iter().flatten().count(), 10, "{:?}", sounds.lines);
    assert_eq!(halo_ce::game::campaign::voice::compile(def, &file, None).filled(), 0);
    eprintln!("elite: footsteps {:?}", &sounds.set.library.footsteps[1][..4.min(sounds.set.library.footsteps[1].len())]);
    let top = m.model.bounds.1.y;
    let tables = BodyTables::build(&data, &MovementData::builtin()).unwrap();
    let mv = &tables.movement[&BodyKind::Elite];
    let env = movement::envelope(mv);
    let v = &tables.vitality[&BodyKind::Elite];
    eprintln!(
        "elite: envelope radius {:.3} m, height {:.3} m, crouch {:.3} m; camera {:.3} / {:.3} m; \
         shield {} body {}; {} hit boxes; model top {:.3} m; body unit {} tags {} bytes (file {} bytes), \
         voice unit {} tags {} bytes (file {} bytes)",
        env.radius,
        env.height,
        env.crouch_height,
        mv.camera_stand * METERS_PER_UNIT,
        mv.camera_crouch * METERS_PER_UNIT,
        v.model.maximum_shield,
        v.model.maximum_body,
        m.hitboxes.len(),
        top,
        c.body_unit.tags.len(),
        c.body_unit.total(),
        c.body.len(),
        c.voice_unit.tags.len(),
        c.voice_unit.total(),
        c.voice.len(),
    );
    assert!((top - 0.786 * METERS_PER_UNIT).abs() < 0.02, "model top {top}");

}

/// The lowest foot bone over a sequence's frames, metres above the body's
/// origin (the root applied).
fn lowest_foot(c: &chimera_world::map::MapCharacterModel, name: &str) -> Option<(f32, f32)> {
    use halo_ce::game::body::{anims, skeleton::globals};
    let set = c.animations.as_ref()?;
    let s = *set.names.get(name)?;
    let a = set.sequences[s].anims[0];
    let n = set.animations[a].frames;
    let feet: Vec<usize> = c.bones.iter().enumerate().filter(|(_, b)| b.name.contains("foot") || b.name.contains("toe")).map(|(i, _)| i).collect();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for f in 0..n {
        let g = globals(&c.bones, &anims::base_pose(set, a, f));
        let low = feet.iter().map(|&i| c.root.transform_point(g[i].1).y).fold(f32::MAX, f32::min);
        lo = lo.min(low);
        hi = hi.max(low);
    }
    Some((lo, hi))
}

/// Where the feet are, standing and moving (stage 2's "feet off the
/// ground"): the Elite's against the Spartan's.
#[test]
fn the_feet_meet_the_ground() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let def = data.body(BodyKind::Elite).unwrap();
    let c = unit::convert("elite", def, &mount.read(&maps::map_path(&def.donor_map)).unwrap()).unwrap();
    let elite = load::biped_body(&MapFile::load(&c.body).unwrap(), &def.biped, false, def.tint(&c.ranks)).unwrap();
    let spartan = load::spartan(&MapFile::load(&mount.read(&maps::map_path(load::DONOR_MAP)).unwrap()).unwrap(), false).unwrap();
    for (who, m, label) in [("elite", &elite.character, "pr"), ("spartan", &spartan.character, "pr")] {
        {
            use halo_ce::game::body::skeleton::globals;
            let set = m.animations.as_ref().unwrap();
            let g = globals(&m.bones, &set.defaults);
            let bind = m.bones.iter().enumerate().filter(|(_, b)| b.name.contains("foot")).map(|(i, _)| m.root.transform_point(g[i].1).y).fold(f32::MAX, f32::min);
            eprintln!("{who} bind pose: lowest foot {bind:.3} m");
            // Standing, the ankles are where the bind pose (feet on the
            // origin) has them; walking, a planted foot comes down to them.
            let (lo, _) = lowest_foot(m, &key("stand", label, "idle")).unwrap();
            assert!((lo - bind).abs() < 0.02, "{who} idle ankle {lo} against {bind}");
            for state in ["move-front", "move-left"] {
                let (lo, hi) = lowest_foot(m, &key("stand", label, state)).unwrap();
                eprintln!("{who} {state}: lowest foot {lo:.3} to {hi:.3} m");
                assert!(lo > bind - 0.05 && lo < bind + 0.02, "{who} {state} planted ankle {lo} against {bind}");
            }
        }
    }
}

/// The Grunt and the Jackal cut from d20 (characters.md section 8: 169
/// tags, 12 153 628 bytes; 173 tags, 8 546 222 bytes), their bodies
/// loading with a head box and, the Jackal's, its disc as a body part and
/// a shield box; their own footsteps and lines.
#[test]
fn the_grunt_and_the_jackal_are_cut_and_load() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let d20 = mount.read(&maps::map_path("d20")).unwrap();
    for (kind, tags, bytes) in [(BodyKind::Grunt, 169, 12_153_628), (BodyKind::Jackal, 173, 8_546_222)] {
        let def = data.body(kind).unwrap();
        let c = unit::convert(kind.id(), def, &d20).unwrap();
        assert_eq!(c.body_unit.tags.len() + c.voice_unit.tags.len(), tags, "{kind:?}");
        assert_eq!(c.body_unit.total() + c.voice_unit.total(), bytes, "{kind:?}");
        let file = MapFile::load(&c.body).unwrap();
        let body = load::biped_body(&file, &def.biped, false, def.tint(&c.ranks)).unwrap();
        let m = &body.character;
        let heads = m.hitboxes.iter().filter(|h| h.region == chimera_core::core::BodyRegion::Head).count();
        let shields = m.hitboxes.iter().filter(|h| h.region == halo_ce::game::body::hits::SHIELD_REGION).count();
        let sounds = halo_ce::game::campaign::voice::compile(def, &file, Some(&MapFile::load(&c.voice).unwrap()));
        let set = m.animations.as_ref().unwrap();
        let label = if kind == BodyKind::Grunt { "ne" } else { "pp" };
        for state in ["idle", "move-front", "move-back", "move-left"] {
            assert!(set.names.contains_key(&key("stand", label, state)), "{kind:?} {state}");
        }
        let discs: Vec<&str> = m.model.meshes.iter().map(|x| x.material.as_str()).filter(|x| x.contains("shield")).collect();
        eprintln!(
            "{kind:?}: body {} tags (file {} bytes), voice {} tags (file {} bytes); {} hit boxes, {heads} head, {shields} shield;              footstep kinds {}, lines {}; meshes {:?}",
            c.body_unit.tags.len(),
            c.body.len(),
            c.voice_unit.tags.len(),
            c.voice.len(),
            m.hitboxes.len(),
            sounds.set.library.footsteps.len(),
            sounds.filled(),
            discs,
        );
        assert_eq!(heads, 1, "{kind:?}");
        assert_eq!(shields, usize::from(kind == BodyKind::Jackal), "{kind:?}");
        assert!(kind != BodyKind::Jackal || !discs.is_empty(), "the Jackal's disc meshes");
        assert!(sounds.filled() >= 6, "{kind:?} lines {:?}", sounds.lines);
    }
}

/// The Marine (characters.md sections 2, 3, 6 and 8): cut from a30, its
/// body with no voice in it and its four voices each a unit of its own
/// (each speaks pain and death lines); its rifle class animates the
/// assault rifle, shotgun and sniper rifle labels; the collision model its
/// vitality row names is the biped's own.
#[test]
fn the_marine_is_cut_with_its_voices_apart() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let def = data.body(BodyKind::Marine).unwrap();
    let a30 = mount.read(&maps::map_path("a30")).unwrap();
    let c = unit::convert("marine", def, &a30).unwrap();
    assert!(c.voice.is_empty() || c.voice_unit.tags.is_empty(), "no voice in one piece");
    assert_eq!(c.voices.len(), 4);
    let file = MapFile::load(&c.body).unwrap();
    let body = load::biped_body(&file, &def.biped, false, def.tint(&c.ranks)).unwrap();
    let set = body.character.animations.as_ref().unwrap();
    for label in ["ar", "sg", "sr", "pr", "ne"] {
        assert!(set.names.contains_key(&key("stand", label, "idle")), "{label}");
    }
    let silent = halo_ce::game::campaign::voice::compile(def, &file, None);
    assert_eq!(silent.filled(), 0, "the body alone is silent");
    for (name, bytes, u) in &c.voices {
        let voice = MapFile::load(bytes).unwrap();
        let lines = halo_ce::game::campaign::voice::compile_lines(def, &file, &voice, Some(name));
        eprintln!("marine voice {name}: {} tags, {} bytes, {} lines", u.tags.len(), u.total(), lines.filled());
        assert!(lines.filled() >= 6, "{name}: {:?}", lines.lines);
    }
    let biped = blam_formats::object::Biped::read(&file, file.find(blam_formats::cache::class::BIPED, &def.biped).unwrap()).unwrap();
    let coll = file.tag(biped.object.collision_model.id).map(|t| t.path.clone());
    let named = def.vitality.get("model").and_then(|m| m.get("tag")).and_then(|t| t.as_str()).map(str::to_string);
    eprintln!("marine: body {} tags, {} bytes; collision model {coll:?}", c.body_unit.tags.len(), c.body_unit.total());
    assert_eq!(coll, named, "its vitality row is its own collision model's");
}

/// Each body standing and aiming level holds its gun along its facing
/// (animations.md "Aiming and looking screens"): the idle's frame 0 with
/// the aiming screen at yaw 0 and pitch 0, the gun's forward (the hand
/// marker's +X on the hand's node) within a few degrees of the body's +X.
#[test]
fn every_body_aims_its_gun_along_its_facing() {
    use bevy::math::{Quat, Vec3};
    use blam_formats::{animation::AnimationGraph, cache::class, model::Model, object::Biped};
    use halo_ce::game::body::anims;
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let mut worst = Vec::new();
    for (kind, donor, label) in [(BodyKind::Elite, "d20", "pr"), (BodyKind::Grunt, "d20", "ne"), (BodyKind::Jackal, "d20", "pp"), (BodyKind::Marine, "a30", "ar")] {
        let def = data.body(kind).unwrap();
        let bytes = mount.read(&maps::map_path(donor)).unwrap();
        let c = unit::convert(kind.id(), def, &bytes).unwrap();
        let file = MapFile::load(&c.body).unwrap();
        let body = load::biped_body(&file, &def.biped, false, def.tint(&c.ranks)).unwrap();
        let set = body.character.animations.as_ref().unwrap();
        let bones = &body.character.bones;
        let biped = Biped::read(&file, file.find(class::BIPED, &def.biped).unwrap()).unwrap();
        let model = Model::read(&file, file.tag(biped.object.model.id).unwrap()).unwrap();
        let graph = AnimationGraph::read(&file, file.tag(biped.object.animation_graph.id).unwrap()).unwrap();
        let (hand_bone, grip) = halo_ce::game::view::load::hand(&model, &graph, label).unwrap();
        let idle = set.sequences[set.names[&key("stand", label, "idle")]].anims[0];
        let mut pose = anims::base_pose(set, idle, 0);
        let aim = set.names[&key("stand", label, "aim-still")];
        let bounds = anims::screen_bounds(&set.sequences[aim]);
        anims::apply_screen(set, aim, 0.0, 0.0, &mut pose);
        let mut global = vec![Quat::IDENTITY; bones.len()];
        for (i, b) in bones.iter().enumerate() {
            global[i] = b.parent.map_or(Quat::IDENTITY, |p| global[p]) * pose[i].0;
        }
        let h = bones.iter().position(|b| b.name == hand_bone).unwrap();
        let (_, grip_q, _) = grip.to_scale_rotation_translation();
        let fwd = global[h] * grip_q * Vec3::X;
        let yaw = fwd.y.atan2(fwd.x).to_degrees();
        let head = bones.iter().position(|b| b.name.ends_with("head")).unwrap();
        let hf = global[head] * Vec3::X;
        eprintln!(
            "{kind:?} ({label}): bounds {bounds:?}; gun forward {fwd:?} yaw {yaw:.0} deg, pitch {:.0}; head +X yaw {:.0}",
            fwd.z.asin().to_degrees(),
            hf.y.atan2(hf.x).to_degrees()
        );
        worst.push((kind, yaw));
    }
    for (kind, yaw) in worst {
        assert!(yaw.abs() < 10.0, "{kind:?} holds its gun {yaw:.0} degrees off its facing");
    }
}

