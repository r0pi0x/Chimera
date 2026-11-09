//! A prop model's prop data (specs/source/prop_damage.md 2.1): the
//! model's `prop_data` block over the `scripts/propdata.txt` template it
//! names (recursively), its fire and physgun interactions, and the pieces
//! its `.phy` says it breaks into (7.3).

use bevy::prelude::*;

use source_formats::keyvalues::{Kv, parse_res};

/// A model's resolved prop data.
#[derive(Clone, Debug, PartialEq)]
pub struct PropData {
    pub health: Option<i32>,
    /// Damage multipliers: bullets, club, blast.
    pub bullets: f32,
    pub club: f32,
    pub explosive: f32,
    pub physicsmode: Option<i32>,
    /// A gib list name (propdata "BreakableModels") and how many chunks.
    pub breakable_model: Option<String>,
    pub breakable_count: i32,
    pub breakable_skin: i32,
    pub damage_table: Option<String>,
    pub explosive_damage: Option<f32>,
    pub explosive_radius: Option<f32>,
    pub allowstatic: bool,
    /// Interactions that matter (spec 2.1 step 4): `flammable`,
    /// `explosive_resist`, `ignite_halfhealth`, `explode_fire`,
    /// `firstimpact_break`, `launch_spin_z`, `launch_spin_none` (the
    /// gravity gun's launch: specs/hl2dm/gravity_gun.md 7).
    pub interactions: Vec<&'static str>,
    /// `physgun_interactions`' `preferred_carryangles` (pitch, yaw, roll):
    /// how the gravity gun holds it (gravity_gun.md 4).
    pub carry_angles: Option<[f32; 3]>,
    /// A template it names doesn't exist.
    pub missing_template: bool,
}

impl Default for PropData {
    fn default() -> Self {
        Self {
            health: None,
            bullets: 1.0,
            club: 1.0,
            explosive: 1.0,
            physicsmode: None,
            breakable_model: None,
            breakable_count: 0,
            breakable_skin: 0,
            damage_table: None,
            explosive_damage: None,
            explosive_radius: None,
            allowstatic: false,
            interactions: Vec::new(),
            carry_angles: None,
            missing_template: false,
        }
    }
}

/// The first block named `name` anywhere under `kv` (depth first).
fn find<'a>(kv: &'a Kv, name: &str) -> Option<&'a Kv> {
    for (k, v) in kv.items() {
        if let Kv::Block(_) = v {
            if k.eq_ignore_ascii_case(name) {
                return Some(v);
            }
            if let Some(f) = find(v, name) {
                return Some(f);
            }
        }
    }
    None
}

fn interactions(block: &Kv, kind: &str, out: &mut Vec<&'static str>) {
    for (k, v) in block.items() {
        let Kv::Value(v) = v else { continue };
        let (k, v) = (k.to_ascii_lowercase(), v.trim().to_ascii_lowercase());
        let found = match (kind, k.as_str(), v.as_str()) {
            ("fire", "flammable", "yes") => "flammable",
            ("fire", "explosive_resist", "yes") => "explosive_resist",
            ("fire", "ignite", "halfhealth") => "ignite_halfhealth",
            ("physgun", "onbreak", "explode_fire") => "explode_fire",
            ("physgun", "onfirstimpact", "break") => "firstimpact_break",
            ("physgun", "onlaunch", "spin_zaxis") => "launch_spin_z",
            ("physgun", "onlaunch", "spin_none") => "launch_spin_none",
            _ => continue,
        };
        if !out.contains(&found) {
            out.push(found);
        }
    }
}

/// A `physgun_interactions` block: its interactions and carry angles.
fn physgun(block: &Kv, pd: &mut PropData) {
    interactions(block, "physgun", &mut pd.interactions);
    if let Some(a) = block.str("preferred_carryangles") {
        let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if let [pitch, yaw, roll] = v[..] {
            pd.carry_angles = Some([pitch, yaw, roll]);
        }
    }
}

/// Apply a prop data block (a template or the model's own): its base
/// first, then its own keys.
fn apply(block: &Kv, templates: &Kv, pd: &mut PropData, depth: u32) {
    if let Some(base) = block.str("base").map(str::trim).filter(|b| !b.is_empty()) {
        match find(templates, base) {
            Some(t) if depth < 16 => apply(t, templates, pd, depth + 1),
            Some(_) => {}
            None => pd.missing_template = true,
        }
    }
    let f = |v: &str| v.trim().parse::<f32>().ok();
    let i = |v: &str| v.trim().parse::<f32>().ok().map(|x| x as i32);
    for (k, v) in block.items() {
        match v {
            Kv::Block(_) => match k.to_ascii_lowercase().as_str() {
                "fire_interactions" => interactions(v, "fire", &mut pd.interactions),
                "physgun_interactions" => physgun(v, pd),
                _ => {}
            },
            Kv::Value(v) => match k.to_ascii_lowercase().as_str() {
                "health" => pd.health = i(v).or(pd.health),
                "dmg.bullets" => pd.bullets = f(v).unwrap_or(pd.bullets),
                "dmg.club" => pd.club = f(v).unwrap_or(pd.club),
                "dmg.explosive" => pd.explosive = f(v).unwrap_or(pd.explosive),
                "physicsmode" => pd.physicsmode = i(v).or(pd.physicsmode),
                "breakable_model" => pd.breakable_model = Some(v.trim().to_string()).filter(|s| !s.is_empty()),
                "breakable_count" => pd.breakable_count = i(v).unwrap_or(pd.breakable_count),
                "breakable_skin" => pd.breakable_skin = i(v).unwrap_or(pd.breakable_skin),
                "damage_table" => pd.damage_table = Some(v.trim().to_string()).filter(|s| !s.is_empty()),
                "explosive_damage" => pd.explosive_damage = f(v).or(pd.explosive_damage),
                "explosive_radius" => pd.explosive_radius = f(v).or(pd.explosive_radius),
                "allowstatic" => pd.allowstatic = i(v).is_some_and(|x| x != 0),
                _ => {}
            },
        }
    }
}

/// propdata.txt's templates.
pub fn templates(text: &str) -> Kv {
    parse_res(text)
}

/// A model's prop data from its key values text, over `templates`
/// (propdata.txt); None when the model has no `prop_data` block.
pub fn resolve(model_kv: &str, templates: &Kv) -> Option<PropData> {
    let kv = parse_res(model_kv);
    let own = find(&kv, "prop_data")?;
    let mut pd = PropData::default();
    apply(own, templates, &mut pd, 0);
    // Interaction blocks beside prop_data.
    for (k, v) in kv.items().iter().chain(kv.items().iter().flat_map(|(_, v)| v.items())) {
        match k.to_ascii_lowercase().as_str() {
            "fire_interactions" => interactions(v, "fire", &mut pd.interactions),
            "physgun_interactions" => physgun(v, &mut pd),
            _ => {}
        }
    }
    Some(pd)
}

/// The templates a breakable brush's "propdata" keyvalue picks, by
/// number 1..=24 (0: none). The spec gives the ends (breakables.md: 1
/// Wooden.Tiny .. 24 Glass.Window); the order between is the Hammer
/// entity definitions' list as the public wiki gives it (docs/tech-debt.md
/// "Breakables").
pub const BRUSH_TEMPLATES: [&str; 24] = [
    "Wooden.Tiny",
    "Wooden.Small",
    "Wooden.Medium",
    "Wooden.Large",
    "Wooden.Huge",
    "Metal.Small",
    "Metal.Medium",
    "Metal.Large",
    "Cardboard.Small",
    "Cardboard.Medium",
    "Cardboard.Large",
    "Stone.Small",
    "Stone.Medium",
    "Stone.Large",
    "Stone.Huge",
    "Glass.Small",
    "Plastic.Small",
    "Plastic.Medium",
    "Plastic.Large",
    "Pottery.Small",
    "Pottery.Medium",
    "Pottery.Large",
    "Pottery.Huge",
    "Glass.Window",
];

/// Brush classes with a "propdata" keyvalue (breakables.md,
/// physics_brushes.md 2).
pub const BRUSH_CLASSES: &[&str] = &[
    "func_breakable",
    "func_breakable_surf",
    "func_physbox",
    "func_physbox_multiplayer",
];

/// A template's prop data by name (its `base` chain first); None when
/// propdata.txt has no such template.
pub fn template(name: &str, templates: &Kv) -> Option<PropData> {
    let block = find(templates, name)?;
    let mut pd = PropData::default();
    apply(block, templates, &mut pd, 0);
    Some(pd)
}

/// What a breakable brush's "propdata" template fills in, as the keys the
/// logic reads (`map::entities::PROP_HEALTH_KEY`, `PROP_DAMAGE_KEY`,
/// `PROP_GIBS_KEY`): the template's health (when it sets one), its
/// bullet/club/blast multipliers, and its chunk list (`breakable_model`,
/// resolved against `lists`) as the gib list. Empty for other classes, no
/// template (0 or out of range) or a template propdata.txt lacks.
pub fn brush_keys(
    classname: &str,
    propdata: i32,
    templates: &Kv,
    lists: &[(String, Vec<String>)],
) -> Vec<(String, String)> {
    use crate::map::entities::{PROP_DAMAGE_KEY, PROP_GIBS_KEY, PROP_HEALTH_KEY};
    if !BRUSH_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(classname)) {
        return Vec::new();
    }
    let Some(name) = usize::try_from(propdata)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| BRUSH_TEMPLATES.get(i))
    else {
        return Vec::new();
    };
    let Some(pd) = template(name, templates) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(h) = pd.health {
        out.push((PROP_HEALTH_KEY.to_string(), h.to_string()));
    }
    out.push((
        PROP_DAMAGE_KEY.to_string(),
        format!("{} {} {}", pd.bullets, pd.club, pd.explosive),
    ));
    if let Some((list, _)) = pd.breakable_model.as_deref().and_then(|m| chunk_list(lists, m)) {
        out.push((PROP_GIBS_KEY.to_string(), list.clone()));
    }
    out
}

/// A gib list for a chunk name: the first list whose name starts with it
/// (spec 2.1).
pub fn chunk_list<'a>(lists: &'a [(String, Vec<String>)], name: &str) -> Option<&'a (String, Vec<String>)> {
    let name = name.to_ascii_lowercase();
    lists.iter().find(|(n, _)| n.to_ascii_lowercase().starts_with(&name))
}

/// The template chunk size limit for a prop box `size` (units, spec 2.1
/// step 5): the smallest axis replaced by 1 (ties pick the later axis),
/// the product over 1024.
pub fn chunk_size_limit(size: Vec3) -> usize {
    let (x, y, z) = (size.x, size.y, size.z);
    let smallest = if x < y && x < z {
        0
    } else if x < y {
        2
    } else if y < z {
        1
    } else {
        2
    };
    let mut s = size;
    s[smallest] = 1.0;
    ((s.x * s.y * s.z) / 1024.0).floor().max(0.0) as usize
}

/// One `.phy` break block (spec 7.3).
#[derive(Clone, Debug, PartialEq)]
pub struct BreakPiece {
    /// Model path, `models/...mdl`.
    pub model: String,
    /// Offset in the prop's model space, units.
    pub offset: Vec3,
    /// Seconds before it fades; 0 never.
    pub fadetime: f32,
    /// Outward speed, units/s.
    pub burst: f32,
    pub motion_disabled: bool,
    /// Debris (spec 7.3: `debris` > 0, else interactive; a `fadetime`
    /// without `debris` makes it debris too): it collides with the world
    /// and props, not with other debris (other pieces, ragdolls). A block
    /// without either key is debris (*inferred*: the spec gives the key no
    /// default; the fade default of 20 s is a debris piece's).
    pub debris: bool,
    /// `fademindist` and `fademaxdist` (units), when both are set and
    /// the max is at least the min (spec 7.3).
    pub fade_dist: Option<(f32, f32)>,
}

/// A `.phy`'s break blocks as pieces.
pub fn break_pieces(blocks: &[Vec<(String, String)>]) -> Vec<BreakPiece> {
    blocks
        .iter()
        .filter_map(|keys| {
            let get = |k: &str| keys.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim());
            let mut model = get("model")?.replace('\\', "/").to_ascii_lowercase();
            if model.is_empty() {
                return None;
            }
            if !model.starts_with("models/") {
                model = format!("models/{model}");
            }
            if !model.ends_with(".mdl") {
                model.push_str(".mdl");
            }
            let num = |k: &str, d: f32| get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
            Some(BreakPiece {
                model,
                offset: get("offset").map_or(Vec3::ZERO, crate::map::entities::parse_vector),
                fadetime: num("fadetime", 20.0),
                burst: num("burst", 100.0),
                motion_disabled: num("motiondisabled", 0.0) != 0.0,
                debris: match get("debris").and_then(|v| v.parse::<f32>().ok()) {
                    Some(d) => d > 0.0 || get("fadetime").is_some(),
                    None => true,
                },
                fade_dist: match (
                    get("fademindist").and_then(|v| v.parse::<f32>().ok()),
                    get("fademaxdist").and_then(|v| v.parse::<f32>().ok()),
                ) {
                    (Some(min), Some(max)) if max >= min => Some((min, max)),
                    _ => None,
                },
            })
        })
        .collect()
}

/// The break sound entry of a surface property (its `break` key through
/// `base`; spec 7.4).
pub fn break_sound(surfaces: &super::surfaceprops::SurfaceProps, surface: &str) -> Option<String> {
    let s = surface.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("default") {
        return None;
    }
    surfaces.text(s, "break").filter(|b| !b.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATES: &str = r#"
"PropData.txt"
{
    "sections"
    {
        "Wooden"
        {
            "Wooden.Base" { "dmg.bullets" "0.75" "dmg.club" "2.0" "dmg.explosive" "1.5" "breakable_model" "WoodChunks" "breakable_skin" "0" }
            "Wooden.Medium" { "base" "Wooden.Base" "health" "30" "breakable_count" "4" }
            "Wooden.Large" { "base" "Wooden.Base" "health" "50" "physicsmode" "1" "breakable_count" "6" }
        }
        "Metal.Base" { "health" "0" }
        "Metal.Medium" { "base" "Metal.Base" "physicsmode" "1" }
    }
    "BreakableModels"
    {
        "WoodChunks" { "models/gibs/wood_gib01e.mdl" "1" "models/gibs/wood_gib01d.mdl" "1" }
    }
}
"#;

    #[test]
    fn templates_resolve_recursively_and_models_override() {
        let t = templates(TEMPLATES);
        let pd = resolve(r#""prop_data" { "base" "Wooden.Medium" }"#, &t).unwrap();
        assert_eq!(pd.health, Some(30));
        assert_eq!((pd.bullets, pd.club, pd.explosive), (0.75, 2.0, 1.5));
        assert_eq!(pd.breakable_model.as_deref(), Some("WoodChunks"));
        assert_eq!(pd.breakable_count, 4);
        let pd = resolve(r#""prop_data" { "base" "Wooden.Large" "health" "20" }"#, &t).unwrap();
        assert_eq!((pd.health, pd.physicsmode), (Some(20), Some(1)));
        let pd = resolve(r#""prop_data" { "base" "Metal.Medium" }"#, &t).unwrap();
        assert_eq!(pd.health, Some(0));
        assert!(!pd.missing_template);
        let pd = resolve(r#""prop_data" { "base" "Nope" }"#, &t).unwrap();
        assert!(pd.missing_template);
        assert!(resolve(r#""physgun_interactions" { }"#, &t).is_none());
    }

    #[test]
    fn brush_propdata_picks_a_template_by_number() {
        use crate::map::entities::{PROP_DAMAGE_KEY, PROP_GIBS_KEY, PROP_HEALTH_KEY};
        let t = templates(TEMPLATES);
        let lists = vec![("WoodChunks".to_string(), vec!["models/gibs/wood_gib01e.mdl".to_string()])];
        // The spec's ends: 1 Wooden.Tiny .. 24 Glass.Window.
        assert_eq!(BRUSH_TEMPLATES[0], "Wooden.Tiny");
        assert_eq!(BRUSH_TEMPLATES[23], "Glass.Window");
        let get = |keys: &[(String, String)], k: &str| keys.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        // 3: Wooden.Medium, through Wooden.Base.
        let keys = brush_keys("func_breakable", 3, &t, &lists);
        assert_eq!(get(&keys, PROP_HEALTH_KEY).as_deref(), Some("30"));
        assert_eq!(get(&keys, PROP_DAMAGE_KEY).as_deref(), Some("0.75 2 1.5"));
        assert_eq!(get(&keys, PROP_GIBS_KEY).as_deref(), Some("WoodChunks"));
        // 7: Metal.Medium: health 0 from Metal.Base, no chunks.
        let keys = brush_keys("func_physbox", 7, &t, &lists);
        assert_eq!(get(&keys, PROP_HEALTH_KEY).as_deref(), Some("0"));
        assert_eq!(get(&keys, PROP_DAMAGE_KEY).as_deref(), Some("1 1 1"));
        assert_eq!(get(&keys, PROP_GIBS_KEY), None);
        // None, out of range, a template the file lacks, other classes.
        assert!(brush_keys("func_breakable", 0, &t, &lists).is_empty());
        assert!(brush_keys("func_breakable", 25, &t, &lists).is_empty());
        assert!(brush_keys("func_breakable", 24, &t, &lists).is_empty());
        assert!(brush_keys("func_door", 3, &t, &lists).is_empty());
    }

    #[test]
    fn the_guns_launch_and_carry_rules() {
        let t = templates(r#""PropData" { }"#);
        let kv = r#""prop_data" { "health" "5" }
            "physgun_interactions" { "onlaunch" "spin_zaxis" "preferred_carryangles" "-90 0 0" }"#;
        let pd = resolve(kv, &t).unwrap();
        assert_eq!(pd.interactions, vec!["launch_spin_z"]);
        assert_eq!(pd.carry_angles, Some([-90.0, 0.0, 0.0]));
        let kv = r#""prop_data" { "health" "5" } "physgun_interactions" { "onlaunch" "spin_none" }"#;
        assert_eq!(resolve(kv, &t).unwrap().interactions, vec!["launch_spin_none"]);
    }

    #[test]
    fn interactions_beside_prop_data() {
        let t = templates(TEMPLATES);
        let text = r#""prop_data" { "base" "Metal.Medium" "health" "20" "explosive_damage" "25" "explosive_radius" "80" }
            "physgun_interactions" { "onbreak" "explode_fire" }
            "fire_interactions" { "flammable" "yes" "ignite" "halfhealth" }"#;
        let pd = resolve(text, &t).unwrap();
        assert_eq!(pd.explosive_damage, Some(25.0));
        assert_eq!(pd.explosive_radius, Some(80.0));
        assert_eq!(pd.interactions, vec!["explode_fire", "flammable", "ignite_halfhealth"]);
    }

    #[test]
    fn chunk_size_limit_and_lists() {
        // woodbarrel001 (spec test case): 28.9 x 28.9 x 40.6 -> 1.
        assert_eq!(chunk_size_limit(Vec3::new(28.9, 28.9, 40.6)), 1);
        assert_eq!(chunk_size_limit(Vec3::new(40.5, 40.3, 40.2)), 1);
        assert_eq!(chunk_size_limit(Vec3::new(100.0, 100.0, 10.0)), 9);
        let lists = vec![("WoodChunks".to_string(), vec!["a".to_string()])];
        assert!(chunk_list(&lists, "wood").is_some());
        assert!(chunk_list(&lists, "Metal").is_none());
    }

    #[test]
    fn break_blocks_become_pieces() {
        let blocks = vec![
            vec![
                ("model".into(), "props_junk/wood_crate001a_chunk01".into()),
                ("fadetime".into(), "10".into()),
            ],
            vec![
                ("model".into(), "models/a.mdl".into()),
                ("motiondisabled".into(), "1".into()),
                ("fademindist".into(), "200".into()),
                ("fademaxdist".into(), "400".into()),
            ],
            vec![
                ("model".into(), "models/b.mdl".into()),
                ("fademindist".into(), "500".into()),
                ("fademaxdist".into(), "400".into()),
                ("debris".into(), "0".into()),
            ],
            vec![
                ("model".into(), "models/c.mdl".into()),
                ("debris".into(), "0".into()),
                ("fadetime".into(), "5".into()),
            ],
        ];
        let p = break_pieces(&blocks);
        assert_eq!(p[0].model, "models/props_junk/wood_crate001a_chunk01.mdl");
        assert_eq!((p[0].fadetime, p[0].burst), (10.0, 100.0));
        assert!(p[1].motion_disabled && p[1].fadetime == 20.0);
        // Debris unless `debris` 0 without a `fadetime`.
        assert_eq!(p.iter().map(|p| p.debris).collect::<Vec<_>>(), [true, true, false, true]);
        // Fade distances need both, max at least min.
        assert_eq!(p[0].fade_dist, None);
        assert_eq!(p[1].fade_dist, Some((200.0, 400.0)));
        assert_eq!(p[2].fade_dist, None);
    }
}
