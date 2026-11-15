//! The collision model's vitality reader on a synthetic `coll` tag with the
//! Spartan's numbers of specs/halo_ce/health_shields.md ("The collision
//! model's vitality fields", the materials and regions tables), the
//! rejections and the formulas on its fields (shield delay and recharge
//! in ticks).

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{MapFile, TagEntry};
use blam_formats::collision::{MaterialType, read_model};
use blam_formats::object::TransitionFunction;
use blam_formats::vitality::{MaterialFlags, ModelFlags, RegionFlags, Vitality};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const SCNR: usize = 0;
const EFFE: usize = 1;
const COLL: usize = 2;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("effe", "characters\\test\\shield depletion"),
        ("coll", "characters\\test\\test"),
    ]
}

fn material(name: &str, flags: u32, ty: i16, leak: f32, shield: f32, body: f32) -> Vec<u8> {
    Struct::new(0x48)
        .str(0, name)
        .u32(0x20, flags)
        .i16(0x24, ty)
        .f32(0x28, leak)
        .f32(0x2C, shield)
        .f32(0x3C, body)
        .into()
}

fn spartan(indirect: i16, shield_type: i16) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let materials = td.block(&[
        material("head", 1, 21, 0.0, 1.0, 1.0),
        material("body", 0, 21, 0.0, 1.0, 1.0),
        material("legs", 0, 21, 0.0, 1.0, 0.8),
        material("shield generator", 0, 21, 0.0, 1.0, 1.0),
    ]);
    let perms = td.block(&[Struct::new(32).str(0, "__base").into()]);
    let regions = td.block(&[Struct::new(0x54)
        .str(0, "cyborg")
        .u32(0x20, 0x12)
        .f32(0x28, 0.25)
        .put(0x38, &td.tag_ref(EFFE))
        .put(0x48, &perms)
        .into()]);
    let nodes = td.block(&[
        Struct::new(0x40)
            .str(0, "bip01 pelvis")
            .i16(0x20, 0)
            .i16(0x22, -1)
            .i16(0x24, -1)
            .i16(0x26, 1)
            .i16(0x32, 0)
            .into(),
        Struct::new(0x40)
            .str(0, "bip01 head")
            .i16(0x20, 0)
            .i16(0x22, 0)
            .i16(0x24, -1)
            .i16(0x26, -1)
            .i16(0x32, 2)
            .into(),
    ]);
    let main = Struct::new(0x298)
        .u32(0x000, 0x7)
        .i16(0x004, indirect)
        .f32(0x008, 75.0)
        .f32(0x00C, 0.5)
        .f32(0x044, 0.1)
        .put(0x070, &null_ref("effe"))
        .f32(0x080, 3.0)
        .put(0x084, &td.tag_ref(EFFE))
        .f32(0x094, 20.0)
        .put(0x098, &null_ref("effe"))
        .put(0x0A8, &null_ref("effe"))
        .f32(0x0B8, -10.0)
        .put(0x0BC, &null_ref("effe"))
        .f32(0x0CC, 75.0)
        .i16(0x0D2, shield_type)
        .i16(0x0EC, 5)
        .f32(0x0F0, 0.3)
        .f32(0x0F4, 0.2)
        .f32(0x108, 2.0)
        .f32(0x10C, 6.0)
        .f32(0x110, 4.0)
        .f32(0x184, 0.5)
        .put(0x188, &null_ref("effe"))
        .put(0x198, &td.tag_ref(EFFE))
        .put(0x1A8, &null_ref("effe"))
        .f32(0x1C0, 0.008333334)
        .put(0x234, &materials)
        .put(0x240, &regions)
        .put(0x28C, &nodes);
    td.set_struct(COLL, &main.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile) -> &TagEntry {
    &map.tags()[COLL]
}

#[test]
fn reads_the_spartans_vitality_fields() {
    let map = spartan(1, 22);
    let v = Vitality::read(&map, tag(&map)).unwrap();
    assert_eq!(v.flags.bits(), 0x7);
    assert!(v.flags.contains(ModelFlags::TAKES_SHIELD_DAMAGE_FOR_CHILDREN));
    assert!(v.flags.contains(ModelFlags::ALWAYS_SHIELDS_FRIENDLY_DAMAGE));
    assert!(!v.flags.contains(ModelFlags::PASSES_AREA_DAMAGE_TO_CHILDREN));
    assert_eq!(v.flags.unnamed_bits(), 0);
    assert_eq!(v.indirect_damage_material, 1);
    assert_eq!(v.indirect_material().unwrap().name, "body");
    assert_eq!(
        (
            v.maximum_body_vitality,
            v.body_system_shock,
            v.friendly_damage_resistance
        ),
        (75.0, 0.5, 0.1)
    );
    assert!(v.localized_damage_effect.is_null());
    assert_eq!(v.area_damage_effect_threshold, 3.0);
    assert_eq!(
        map.tag(v.area_damage_effect.id).unwrap().path,
        "characters\\test\\shield depletion"
    );
    assert_eq!(v.body_damaged_threshold, 20.0);
    assert!(v.body_damaged_effect.is_null() && v.body_depleted_effect.is_null() && v.body_destroyed_effect.is_null());
    assert_eq!(v.body_destroyed_threshold, -10.0);
    assert_eq!(v.maximum_shield_vitality, 75.0);
    assert!(v.has_shield());
    assert_eq!(v.shield_material_type(), Some(MaterialType::CyborgEnergyShield));
    assert_eq!(v.shield_failure_function, TransitionFunction::Cosine);
    assert_eq!((v.shield_failure_threshold, v.failing_shield_leak_fraction), (0.3, 0.2));
    assert_eq!(v.minimum_stun_damage, 2.0);
    assert_eq!((v.stun_time, v.recharge_time), (6.0, 4.0));
    assert_eq!(v.shield_damaged_threshold, 0.5);
    assert!(v.shield_damaged_effect.is_null() && v.shield_recharging_effect.is_null());
    assert_eq!(map.tag(v.shield_depleted_effect.id).unwrap().class.to_string(), "effe");
    assert_eq!(v.runtime_shield_recharge_velocity, 0.008333334);
    // 6.0 s is 180 ticks, 4.0 s is 120; the rate is 1 / (4 x 30).
    assert_eq!((v.stun_ticks(), v.recharge_ticks()), (180, 120));
    assert!((v.computed_recharge_velocity() - v.runtime_shield_recharge_velocity).abs() < 1e-9);
}

#[test]
fn reads_materials_regions_and_nodes() {
    let map = spartan(1, 22);
    let v = Vitality::read(&map, tag(&map)).unwrap();
    assert_eq!(v.materials.len(), 4);
    let rows = [
        ("head", true, 1.0),
        ("body", false, 1.0),
        ("legs", false, 0.8),
        ("shield generator", false, 1.0),
    ];
    for (i, (name, head, body)) in rows.into_iter().enumerate() {
        let m = v.material(i).unwrap();
        assert_eq!(m.name, name);
        assert_eq!(m.is_head(), head, "{name}");
        assert_eq!(m.flags.contains(MaterialFlags::HEAD), head);
        assert_eq!(m.material_type(), Some(MaterialType::Cyborg));
        assert_eq!(
            (
                m.shield_leak_fraction,
                m.shield_damage_multiplier,
                m.body_damage_multiplier
            ),
            (0.0, 1.0, body)
        );
    }
    assert!(v.material(4).is_none());
    assert_eq!(v.regions.len(), 1);
    let r = v.region(0).unwrap();
    assert_eq!(r.name, "cyborg");
    assert!(r.flags.contains(RegionFlags::FORCES_OBJECT_TO_DIE));
    assert!(r.flags.contains(RegionFlags::DISAPPEARS_WHEN_SHIELD_IS_OFF));
    assert!(!r.flags.contains(RegionFlags::LIVES_UNTIL_OBJECT_DIES));
    assert_eq!(r.damage_threshold, 0.25);
    assert!(!r.destroyed_effect.is_null());
    assert_eq!(
        v.nodes
            .iter()
            .map(|n| (n.name.as_str(), n.damage_part))
            .collect::<Vec<_>>(),
        [("bip01 pelvis", 0), ("bip01 head", 2)]
    );
    // The geometry reader still reads the same tag.
    let m = read_model(&map, tag(&map)).unwrap();
    assert_eq!((m.materials.len(), m.regions.len(), m.nodes.len()), (4, 1, 2));
    assert_eq!(m.regions[0].permutations, ["__base"]);
}

#[test]
fn refuses_damage() {
    // An indirect material past the materials, a tag of another class.
    let map = spartan(4, 22);
    let err = Vitality::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("indirect damage material"), "{err}");
    assert!(err.to_string().contains("characters\\test\\test"), "{err}");
    let map = spartan(-1, 22);
    assert!(Vitality::read(&map, tag(&map)).unwrap().indirect_material().is_none());
    let err = Vitality::read(&map, &map.tags()[EFFE]).unwrap_err();
    assert!(err.to_string().contains("not a collision model"), "{err}");
    // A shield type outside 0 to 32 reads as none.
    let map = spartan(1, 40);
    assert_eq!(Vitality::read(&map, tag(&map)).unwrap().shield_material_type(), None);
}
