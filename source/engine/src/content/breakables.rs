//! CS:S's breakable brushes (specs/source/breakables.md): the gib lists
//! from the install's `scripts/propdata.txt` ("BreakableModels": WoodChunks,
//! GlassChunks, MetalChunks, ConcreteChunks, plus any list or model a
//! map's `gibmodel` names) and how gibs fly. The breaking itself is the
//! logic layer's (`logic::breakables`).

use super::material::MaterialLoader;
use crate::map::{
    MapEntity, MapMesh, MapModel, PaneLook,
    breakables::{MapGibPhysics, MapGibSet, PANE_PIECES},
};

const UNIT: f32 = 0.0254;
/// Glass gibs: alpha 128 of 255, bounce 0.3.
const GLASS_ALPHA: f32 = 128.0 / 255.0;
const GLASS_BOUNCE: f32 = 0.3;
/// Other gibs' bounce (not in the spec: a guess).
const BOUNCE: f32 = 0.5;

/// Brush entity classes that break (logic `Breakable`): drawn and solid
/// through their own node so they can disappear.
pub const CLASSES: &[&str] = &["func_breakable", "func_breakable_surf"];

/// What a bullet or knife hit leaves on a breakable brush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactMark {
    /// The surface's own decal.
    Surface,
    /// This decal group.
    Group(&'static str),
    /// None: a glass window's first hit breaks it (breakables.md "Taking a
    /// hit") and from then on the client draws its panes, not the face the
    /// decal would be on ("What the client draws").
    Nothing,
}

/// What a bullet or knife hit leaves on a breakable brush (specs/source/
/// breakables.md "func_breakable: keyvalues and spawn"): glass (material
/// 0) "GlassBreak", unbreakable glass (material 7) "BulletProof";
/// func_breakable_surf glass nothing.
pub fn impact_decal(classname: &str, material: i32, surfacetype: i32) -> ImpactMark {
    let surf = classname.eq_ignore_ascii_case("func_breakable_surf");
    match material {
        _ if surf && surfacetype != 1 => ImpactMark::Nothing,
        0 => ImpactMark::Group("GlassBreak"),
        7 => ImpactMark::Group("BulletProof"),
        _ => ImpactMark::Surface,
    }
}

/// The gib lists every breakable material uses.
const LISTS: &[&str] = &["WoodChunks", "GlassChunks", "MetalChunks", "ConcreteChunks"];

/// How gibs move, in a game whose `sv_gravity` is `gravity` (units/s²):
/// gibs fall at half of it, broken props' pieces at all of it.
pub fn gib_physics(gravity: f32) -> MapGibPhysics {
    MapGibPhysics {
        gravity: gravity / 2.0 * UNIT,
        bounce: BOUNCE,
        glass_bounce: GLASS_BOUNCE,
        glass_alpha: GLASS_ALPHA,
        fade: 1.0,
        // Broken props' pieces are client physics props: full gravity.
        prop_gravity: gravity * UNIT,
        // The shared temporary-entity bounce sound rule (view_models.md
        // 8, which breakables.md "Breaking" 5 points to by its break
        // flag): one bounce in six, full volume at 450 units/s down, the
        // entry's pitch range one time in four.
        sound_chance: 1.0 / 6.0,
        sound_full_speed: 450.0 * UNIT,
        sound_random_pitch: 0.25,
    }
}

/// The model paths of each "BreakableModels" list in propdata.txt.
pub fn breakable_models(text: &str) -> Vec<(String, Vec<String>)> {
    let kv = source_formats::keyvalues::parse_res(text);
    // The file is one block ("PropData.txt" { ... }) holding the lists.
    let root = kv.items().first().map(|(_, v)| v);
    let Some(lists) = root
        .and_then(|r| r.get("BreakableModels"))
        .or_else(|| kv.get("BreakableModels"))
    else {
        return Vec::new();
    };
    lists
        .items()
        .iter()
        .map(|(name, models)| {
            (
                name.clone(),
                models.items().iter().map(|(path, _)| path.clone()).collect(),
            )
        })
        .collect()
}

/// The falling piece a collapsing window pane drops (specs/source/
/// breakables.md "Support pass"), one model per body (0..2).
pub const PANE_PIECE_MODEL: &str = "models/brokenglass_piece.mdl";

/// The jagged edge materials a broken glass window draws where unbroken
/// panes meet broken ones (the spec names "glassbroken_*"; these are the
/// install's edge textures, a shard rising from the texture's bottom).
pub const GLASS_EDGES: [&str; 12] = [
    "models/brokenglass/glassbroken_01a",
    "models/brokenglass/glassbroken_01b",
    "models/brokenglass/glassbroken_01c",
    "models/brokenglass/glassbroken_01d",
    "models/brokenglass/glassbroken_02a",
    "models/brokenglass/glassbroken_02b",
    "models/brokenglass/glassbroken_02c",
    "models/brokenglass/glassbroken_02d",
    "models/brokenglass/glassbroken_03a",
    "models/brokenglass/glassbroken_03b",
    "models/brokenglass/glassbroken_03c",
    "models/brokenglass/glassbroken_03d",
];

/// Each window's broken looks (`MapMesh::pane_look`): its face again in
/// the face material's `$crackmaterial`, and, for glass, that cracked face
/// as a template for each edge piece, cut by the edge textures' alpha
/// (one texture of them all, `MapMesh::pane_mask`, so all of a crack
/// material's pieces share one material). Both are double sided (the spec leaves which
/// side the client draws open). No crack material: no broken look.
pub fn add_window_looks(materials: &mut MaterialLoader, entities: &[MapEntity], meshes: &mut Vec<MapMesh>) {
    let windows: Vec<(usize, bool)> = entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.classname() == "func_breakable_surf")
        .map(|(i, e)| (i, e.get("surfacetype") == Some("1")))
        .collect();
    if windows.is_empty() {
        return;
    }
    // The edge masks as one texture, so a material's broken looks share one
    // material (`MapMesh::pane_mask`): None without any glass window.
    let masks = windows.iter().any(|(_, tile)| !tile).then(|| edge_mask_atlas(materials)).flatten();
    let mut extra = Vec::new();
    for m in meshes.iter() {
        let Some(&(_, tile)) = m.entity.and_then(|i| windows.iter().find(|(w, _)| *w == i)) else {
            continue;
        };
        if m.pane_look != PaneLook::Whole || m.texture.is_none() {
            continue;
        }
        let Some(crack) = materials.material_value(&m.material, "$crackmaterial") else {
            continue;
        };
        let crack = crack.replace('\\', "/");
        let r = materials.resolve(&crack);
        if r.texture.is_none() {
            continue;
        }
        let cracked = MapMesh {
            material: crack,
            texture: r.texture,
            normal_map: r.normal_map,
            phong: r.phong,
            detail: r.detail,
            base_transform: r.base_transform,
            selfillum: r.selfillum,
            alpha: r.alpha,
            double_sided: true,
            envmap: r.envmap,
            tint: r.tint,
            unlit: r.unlit,
            unlit_factor: r.unlit_factor,
            pane_look: PaneLook::Cracked,
            pane_mask: masks.map(|(t, _)| t),
            ..m.clone()
        };
        // Glass edges: the cracked face again, cut by each jagged mask (the
        // edge textures are white with the shape in their alpha, so the
        // pieces continue the crack texture past the pane's edge).
        if let Some((_, kinds)) = masks.filter(|_| !tile) {
            for k in 0..kinds {
                extra.push(MapMesh {
                    pane_look: PaneLook::Edge(k),
                    ..cracked.clone()
                });
            }
        }
        extra.push(cracked);
    }
    meshes.extend(extra);
}

/// The glass edge textures that load, as square tiles side by side in one
/// new texture (the first's size; others are skipped), and how many.
fn edge_mask_atlas(materials: &mut MaterialLoader) -> Option<(usize, u8)> {
    let tiles: Vec<usize> = GLASS_EDGES
        .iter()
        .filter_map(|name| materials.resolve(name).texture)
        .collect();
    // Texels are read below.
    materials.finish_textures();
    let lean = materials.lean;
    let first = materials.textures.get(*tiles.first()?)?;
    let size = first.width;
    let srgb = first.srgb;
    let tiles: Vec<&crate::map::MapTexture> = tiles
        .iter()
        .map(|&i| &materials.textures[i])
        .filter(|t| t.width == size && t.height == size && (lean || t.rgba8.len() == (size * size * 4) as usize))
        .collect();
    let n = tiles.len();
    let row = size as usize * 4;
    let mut rgba8 = if lean { Vec::new() } else { vec![0u8; row * n * size as usize] };
    for (k, t) in tiles.iter().enumerate().filter(|_| !lean) {
        for y in 0..size as usize {
            let to = y * row * n + k * row;
            rgba8[to..to + row].copy_from_slice(&t.rgba8[y * row..(y + 1) * row]);
        }
    }
    materials.textures.push(crate::map::MapTexture {
        name: "models/brokenglass/glassbroken_edges".into(),
        srgb,
        mips: Vec::new(),
        width: size * n as u32,
        height: size,
        rgba8,
        bc: None,
    });
    Some((materials.textures.len() - 1, n as u8))
}

/// The falling pane piece's models, one per body.
fn pane_pieces(materials: &mut MaterialLoader) -> Result<Vec<MapModel>, String> {
    let model = super::props::load_shell(materials, PANE_PIECE_MODEL)?;
    Ok((0..crate::map::breakables::PANE_PIECE_BODIES as i32)
        .map(|body| {
            let mut m = model.clone();
            m.meshes
                .retain(|mesh| mesh.body.is_none_or(|(part, choice)| model.body_choice(body, part as usize) == choice));
            m.body_parts.clear();
            m
        })
        .collect())
}

/// Breakable brushes that name a "propdata" template get what it fills in
/// (`propdata::brush_keys`) from the install's `scripts/propdata.txt`.
pub fn apply_brush_templates(materials: &mut MaterialLoader, entities: &mut [MapEntity]) {
    let named = |e: &MapEntity| {
        super::propdata::BRUSH_CLASSES.contains(&e.classname()) && e.get("propdata").is_some_and(|p| p.trim() != "0")
    };
    if !entities.iter().any(named) {
        return;
    }
    let text = materials
        .read("scripts/propdata.txt")
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let (templates, lists) = (super::propdata::templates(&text), breakable_models(&text));
    for e in entities.iter_mut().filter(|e| named(e)) {
        let index = e.get("propdata").map_or(0, |p| p.trim().parse::<f32>().map_or(0, |x| x as i32));
        let keys = super::propdata::brush_keys(e.classname(), index, &templates, &lists);
        e.keyvalues.extend(keys);
    }
}

/// Load the gib lists the map's breakables can use.
pub fn load_gibs(materials: &mut MaterialLoader, entities: &[MapEntity], warnings: &mut Vec<String>) -> Vec<MapGibSet> {
    if !entities.iter().any(|e| CLASSES.contains(&e.classname())) {
        return Vec::new();
    }
    let mut out = Vec::new();
    if entities.iter().any(|e| e.classname() == "func_breakable_surf") {
        match pane_pieces(materials) {
            Ok(models) => out.push(MapGibSet {
                name: PANE_PIECES.to_string(),
                models,
            }),
            Err(e) => warnings.push(e),
        }
    }
    let lists = materials
        .read("scripts/propdata.txt")
        .map(|b| breakable_models(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    let mut wanted: Vec<String> = LISTS.iter().map(|s| s.to_string()).collect();
    for e in entities.iter().filter(|e| e.classname() == "func_breakable") {
        // Its own list or model, and its propdata template's list.
        for g in [e.get("gibmodel"), e.get(crate::map::entities::PROP_GIBS_KEY)]
            .into_iter()
            .flatten()
            .filter(|g| !g.is_empty())
        {
            if !wanted.iter().any(|w| w.eq_ignore_ascii_case(g)) {
                wanted.push(g.to_string());
            }
        }
    }
    for name in wanted {
        // A list name, or a single model.
        let paths: Vec<String> = match lists.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            Some((_, paths)) => paths.clone(),
            None if name.to_ascii_lowercase().ends_with(".mdl") => vec![name.clone()],
            None => {
                warnings.push(format!("propdata: no gib list {name}"));
                continue;
            }
        };
        let models = paths
            .iter()
            .filter_map(|p| match super::props::load_shell(materials, p) {
                Ok(m) => Some(m),
                Err(e) => {
                    warnings.push(e);
                    None
                }
            })
            .collect();
        out.push(MapGibSet { name, models });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_takes_glass_break_decals_and_bulletproof_glass_its_own() {
        use ImpactMark::*;
        assert_eq!(impact_decal("func_breakable", 0, 0), Group("GlassBreak"));
        assert_eq!(impact_decal("func_breakable", 7, 0), Group("BulletProof"));
        assert_eq!(impact_decal("func_breakable", 1, 0), Surface);
        assert_eq!(impact_decal("func_breakable", 2, 0), Surface);
        // A glass window, whatever its "material", shows no bullet decal:
        // the hit broke it, its panes are drawn instead of its face. Tile
        // by its material.
        assert_eq!(impact_decal("func_breakable_surf", 2, 0), Nothing);
        assert_eq!(impact_decal("func_breakable_surf", 1, 1), Surface);
    }

    #[test]
    fn reads_breakable_model_lists() {
        let text = r#"
"PropData.txt"
{
    "sections" { "Wooden.Tiny" { "health" "1" } }
    "BreakableModels"
    {
        "MetalChunks"
        {
            "models/gibs/metal_gib1.mdl"  "1"
            "models/gibs/metal_gib2.mdl"  "1"
        }
        "GlassChunks" { "models/gibs/glass_shard01.mdl" "1" }
    }
}
"#;
        let lists = breakable_models(text);
        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].0, "MetalChunks");
        assert_eq!(
            lists[0].1,
            vec!["models/gibs/metal_gib1.mdl", "models/gibs/metal_gib2.mdl"]
        );
        assert_eq!(lists[1].1.len(), 1);
    }
}
