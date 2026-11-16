//! The Hunter and our own lines against the disc (`game::campaign`,
//! specs/halo_ce/characters.md sections 3, 6, 7 and 8): the Hunter cut
//! from d20 is the spec's closure (128 tags, 4 590 526 bytes) and its
//! cannon's unit the spec's (81 tags, 1 016 094 bytes); its body loads with
//! its graph's unarmed `fr` stances (idle, aim, fire, melee) and skin
//! boxes as heads; the cannon's data and its two damage effects are the
//! tags' numbers; every character's chosen becoming and kill lines are
//! filled in its voice set as cut (no set widened: a voice set holds its
//! whole dialogue); the first-person view keeps the cannon arm. Prints
//! the measured numbers (the Hunter's envelope, the arm's place in the
//! view). Passes with a note without an install.

use blam_formats::{
    cache::{MapFile, class},
    damage_effect::DamageEffect,
    projectile::Projectile,
    weapon::Weapon,
};
use halo_ce::game::{
    METERS_PER_UNIT,
    body::{anims::key, load},
    campaign::{BodyKind, BodyTables, CampaignData, first_person, unit, voice},
    maps,
    movement::{self, MovementData},
    vitality::{DamageEffects, ids},
    weapons::{self, rung::HUNTER_RUNG},
};

use super::maps::mount;

#[test]
fn the_hunter_is_cut_from_d20_with_its_cannon_and_loads() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let def = data.body(BodyKind::Hunter).unwrap();
    let d20 = mount.read(&maps::map_path("d20")).unwrap();
    let c = unit::convert("hunter", def, &d20).unwrap();
    assert_eq!(c.body_unit.tags.len() + c.voice_unit.tags.len(), 128);
    assert_eq!(c.body_unit.total() + c.voice_unit.total(), 4_590_526);
    let (tag, bytes, w) = &c.weapons[0];
    assert_eq!((tag.as_str(), w.tags.len(), w.total()), (r"weapons\fuel rod gun\hunter fuel rod", 81, 1_016_094));
    assert_eq!(unit::weapon_path("hunter", tag), "chimera/gen/halo_ce.character_unit/v1/hunter/weapon_hunter_fuel_rod.map");
    eprintln!(
        "hunter: body {} tags {} bytes (file {}), voice {} tags {} bytes (file {}), cannon {} tags {} bytes (file {})",
        c.body_unit.tags.len(),
        c.body_unit.total(),
        c.body.len(),
        c.voice_unit.tags.len(),
        c.voice_unit.total(),
        c.voice.len(),
        w.tags.len(),
        w.total(),
        bytes.len()
    );
    let file = MapFile::load(&c.body).unwrap();
    let body = load::biped_body(&file, &def.biped, false, def.tint()).unwrap();
    let m = &body.character;
    let set = m.animations.as_ref().unwrap();
    for slot in ["idle", "aim-still", "fire-1", "melee", "move-front"] {
        assert!(set.names.contains_key(&key("stand", "fr", slot)), "stand fr {slot}");
    }
    let regions: Vec<String> = m.hitboxes.iter().map(|h| format!("{:?}", h.region)).collect();
    let heads = m.hitboxes.iter().filter(|h| h.region == chimera_core::core::BodyRegion::Head).count();
    eprintln!("hunter: {} hit boxes {regions:?}; model top {:.3} m", m.hitboxes.len(), m.model.bounds.1.y);
    assert!(heads >= 1, "its skin is a head");
    let sounds = voice::compile(def, &file, Some(&MapFile::load(&c.voice).unwrap()));
    assert!(sounds.set.library.footsteps.len() >= 4, "its own footsteps");
    let tables = BodyTables::build(&data, &MovementData::builtin()).unwrap();
    let env = movement::envelope(&tables.movement[&BodyKind::Hunter]);
    eprintln!("hunter: envelope radius {:.3} m height {:.3} m crouch {:.3} m", env.radius, env.height, env.crouch_height);
    // Its first-person view: the cannon arm and the shield forearm kept.
    let fp = def.first_person.as_ref().unwrap();
    let eye = tables.movement[&BodyKind::Hunter].camera_stand * METERS_PER_UNIT;
    let v = first_person::view_model(m, fp, HUNTER_RUNG, eye);
    let kept: usize = v.model.meshes.iter().map(|x| x.indices.len() / 3).sum();
    let all: usize = m.model.meshes.iter().map(|x| x.indices.len() / 3).sum();
    eprintln!("hunter view: {kept} of {all} triangles kept, {} meshes", v.model.meshes.len());
    assert!(kept > 50 && kept * 2 < all, "the arms alone, not the body");
    // Where the arm's bones are in the view, at rest (the level aim).
    let pose = first_person::pose(set, set.names.get(&key("stand", "fr", "idle")).copied(), 0, None, set.names.get(&key("stand", "fr", "aim-still")).copied());
    let mut global = vec![bevy::math::Mat4::IDENTITY; m.bones.len()];
    for (i, b) in m.bones.iter().enumerate() {
        let local = bevy::math::Mat4::from_rotation_translation(pose[i].0, pose[i].1);
        global[i] = b.parent.map_or(bevy::math::Mat4::IDENTITY, |p| global[p]) * local;
    }
    let root = v.root.to_matrix();
    for name in ["bip01 r upperarm", "bip01 r forearm", "bip01 r hand", "bip01 l forearm"] {
        let i = m.bones.iter().position(|b| b.name == name).unwrap();
        eprintln!("hunter view: {name} at {:?} (eye space, m)", root.transform_point3(global[i].transform_point3(bevy::math::Vec3::ZERO)));
    }
    let f = m.bones.iter().position(|b| b.name == "bip01 r forearm").unwrap();
    eprintln!(
        "hunter view: primary trigger at {:?} (eye space, m)",
        root.transform_point3(global[f].transform_point3(bevy::math::Vec3::new(0.39764538, 0.01516785, 0.00482882)))
    );
}

/// The cannon's data (data/weapons/hunter_fuel_rod.toml) and its damage
/// effects (data/damage.toml) are the d20 tags' numbers.
#[test]
fn the_cannon_is_the_tags() {
    let Some(mount) = mount() else { return };
    let map = MapFile::load(&mount.read(&maps::map_path("d20")).unwrap()).unwrap();
    let set = chimera_sim::weapon::data::WeaponSet::parse(weapons::DATA, &[]).unwrap();
    let g: weapons::params::Gun = set.get(HUNTER_RUNG).unwrap().hooks[weapons::WEAPON_HOOK].clone().try_into().unwrap();
    let w = Weapon::read(&map, map.find(class::WEAPON, &g.tag).unwrap()).unwrap();
    let (t, dt) = (&g.trigger, &w.own.triggers[0]);
    assert_eq!((w.own.label.as_str(), w.own.flags.bits(), dt.flags.bits()), (g.label.as_str(), g.flags, t.flags));
    assert_eq!([dt.rounds_per_second.lower, dt.rounds_per_second.upper], t.rounds_per_second);
    assert_eq!((dt.charging_time, dt.charged_time, dt.overcharged_action as u16), (t.charging_time, t.charged_time, t.overcharged_action));
    assert_eq!(dt.first_person_offset, t.first_person_offset);
    assert_eq!(w.own.magazines[0].rounds_loaded_maximum, 0);
    assert!(w.own.first_person_model.id.is_none() && w.own.melee_damage.id.is_none(), "no view model, no melee of its own");
    let p = Projectile::read(&map, map.find(class::PROJECTILE, &t.projectile).unwrap()).unwrap().own;
    let q = &g.projectile;
    assert_eq!((p.initial_velocity, p.final_velocity, p.air_gravity_scale, p.maximum_range), (q.initial_velocity, q.final_velocity, q.air_gravity_scale, q.maximum_range));
    assert_eq!((p.flags.bits(), p.detonation_timer_starts as u16, p.material_responses.len()), (q.flags, q.timer_starts, q.responses.count as usize));
    let fx = DamageEffects::load();
    for id in ids::CAMPAIGN {
        let ours = fx.get(id).unwrap();
        let d = DamageEffect::read(&map, map.find(class::DAMAGE_EFFECT, &ours.tag).unwrap()).unwrap();
        assert_eq!((d.damage_lower_bound, [d.damage_upper_bound.lower, d.damage_upper_bound.upper]), (ours.lower, ours.upper), "{id:?}");
        assert_eq!(([d.radius.lower, d.radius.upper], d.instantaneous_acceleration, d.damage_flags.bits()), (ours.radius, ours.accel, ours.flags), "{id:?}");
        assert_eq!(d.category.code() as u16, ours.category.code(), "{id:?}");
        for m in 0..33 {
            assert_eq!(d.material_modifiers.0[m], ours.modifiers[m], "{id:?} type {m}");
        }
    }
}

/// Every character's chosen lines are filled in its voice as cut.
#[test]
fn every_chosen_line_is_in_its_voice_set() {
    let Some(mount) = mount() else { return };
    let data = CampaignData::builtin();
    let d20 = mount.read(&maps::map_path("d20")).unwrap();
    let a30 = mount.read(&maps::map_path("a30")).unwrap();
    for kind in [BodyKind::Elite, BodyKind::Grunt, BodyKind::Jackal, BodyKind::Hunter, BodyKind::Marine] {
        let def = data.body(kind).unwrap();
        let c = unit::convert(kind.id(), def, if kind == BodyKind::Marine { &a30 } else { &d20 }).unwrap();
        let body = MapFile::load(&c.body).unwrap();
        let voices: Vec<(String, voice::BodySounds, usize)> = if def.voices.is_empty() {
            let v = MapFile::load(&c.voice).unwrap();
            vec![(kind.id().to_string(), voice::compile_lines(def, &body, &v, None), c.voice.len())]
        } else {
            c.voices.iter().map(|(n, b, _)| (n.clone(), voice::compile_lines(def, &body, &MapFile::load(b).unwrap(), Some(n)), b.len())).collect()
        };
        let lines = def.lines.as_ref().unwrap();
        for (name, s, size) in voices {
            let becoming = voice::first_filled(&s, &lines.becoming);
            let kill = voice::first_filled(&s, &lines.kill);
            eprintln!(
                "{name}: becoming {:?}, kill {:?}; voice file {size} bytes",
                becoming.map(voice::slot_name),
                kill.map(voice::slot_name)
            );
            assert!(becoming.is_some() && kill.is_some(), "{name}");
        }
    }
}
