//! The Elite's units from the disc (`game::campaign::unit`,
//! specs/halo_ce/characters.md sections 3, 5 and 8): cut from d20, the
//! body and the voice together are the spec's closure (161 tags, 6 682 822
//! bytes); the body's file loads as a map whose Elite becomes a character
//! model (its height, a head hit box among its boxes, its graph's plasma
//! rifle and needler stances, its deaths) with no voice in it; its tables
//! are the spec's numbers. Prints the measured numbers. Passes with a
//! note without an install.

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
    let body = load::biped_body(&file, &def.biped, false, def.tint()).unwrap();
    let m = &body.character;
    // Its armour takes the player's colour through slot A (every armour
    // shader), and a player given none wears the minor's blue.
    let skins = &m.model.skins;
    assert_eq!(skins.len(), halo_ce::game::body::COLOURS.len() + 1);
    let tinted = skins[0].iter().filter(|l| l.tint.is_some()).count();
    assert!(tinted * 2 > skins[0].len(), "{tinted} of {} meshes tinted", skins[0].len());
    assert!(skins.last().unwrap().iter().any(|l| l.tint == Some([0.314, 0.302, 0.533])));
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
