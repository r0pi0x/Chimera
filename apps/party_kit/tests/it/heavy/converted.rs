//! Each game's guest dressing as a converted asset
//! (docs/plans/active/converted-cache.md): converted twice from the same
//! files it serializes (postcard, as the converted-map cache writes it) to
//! the same bytes, the generated-assets rule's "same input, same output"
//! (step 1: the `HashMap`s a map holds are written in key order,
//! chimera_world's `pod_serde::sorted`); a dressing kept for the session
//! (`map::BlocksOnly`) skips decoding its whole-DXT textures and keeps
//! the same textures (step 2).
//!
//! Each game passes with a note without its install (CS:S, HL2:DM, TF2,
//! Combat Arms with its keys).

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapData;

/// The guests whose install is here, each with its dressing of an empty
/// map (textures decoded, its kept guns), as its kept dressing converts
/// it.
fn guests() -> Vec<(&'static str, Box<dyn Fn(&mut MapData)>)> {
    let mut out: Vec<(&'static str, Box<dyn Fn(&mut MapData)>)> = Vec::new();
    if cs_source::game::INFO.install().is_some() {
        out.push((
            cs_source::game::GAME,
            Box::new(move |m: &mut MapData| {
                cs_source::game::dress_guest_with(m, cs_source::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    if hl2dm::game::INFO.install().is_some() {
        out.push((
            hl2dm::game::GAME,
            Box::new(move |m: &mut MapData| {
                hl2dm::game::maps::dress_guest_with(m, hl2dm::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    if tf2::game::INFO.install().is_some_and(|p| chimera_core::mount::served::usable("tf2", p, "tf")) {
        out.push((
            tf2::game::GAME,
            Box::new(move |m: &mut MapData| {
                tf2::game::maps::dress_guest_with(m, tf2::game::BUNDLE.kept, false).unwrap();
            }),
        ));
    }
    let ca = LocalConfig::load()
        .ok()
        .and_then(|c| c.game_path(combat_arms::game::GAME))
        .is_some_and(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    if ca {
        out.push((
            combat_arms::game::GAME,
            Box::new(move |m: &mut MapData| {
                let data = combat_arms::game::data::CaData::install();
                let mount = data.mount().expect("Combat Arms' install mounts");
                combat_arms::game::world::dress(&data, &mount, m);
            }),
        ));
    }
    out
}

/// The parts of a map's bytes, in the order they're written, for a
/// failure that says where two encodings part.
fn parts(map: &MapData) -> Vec<(&'static str, Vec<u8>)> {
    let enc = |v: &dyn erased::Ser| v.bytes();
    vec![
        ("textures", enc(&map.textures)),
        ("sounds", enc(&*map.sounds)),
        ("kit", enc(&map.kit)),
        ("particles", enc(&map.particles)),
        ("impacts", enc(&map.impacts)),
        ("surface_names", enc(&map.surface_names)),
    ]
}

#[test]
fn a_guest_dressing_converted_twice_writes_the_same_bytes() {
    let guests = guests();
    if guests.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM, TF2 or Combat Arms");
        return;
    }
    for (game, dress) in guests {
        let run = || {
            let mut m = MapData::default();
            dress(&mut m);
            m
        };
        let (a, b) = (run(), run());
        for ((name, x), (_, y)) in parts(&a).iter().zip(parts(&b)) {
            let first = x.iter().zip(&y).position(|(p, q)| p != q);
            assert!(
                x.len() == y.len() && first.is_none(),
                "{game}: its {name} encode differently ({} and {} bytes, first difference at {first:?})",
                x.len(),
                y.len()
            );
        }
        let (x, y) = (postcard::to_allocvec(&a).unwrap(), postcard::to_allocvec(&b).unwrap());
        assert!(x == y, "{game}: the whole dressing encodes differently ({} and {} bytes)", x.len(), y.len());
        eprintln!("{game}: {} bytes, the same twice", x.len());
    }
}

mod erased {
    /// A value's postcard bytes.
    pub trait Ser {
        fn bytes(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn bytes(&self) -> Vec<u8> {
            postcard::to_allocvec(self).unwrap()
        }
    }
}

/// The Source guests' dressings as kept (`map::BlocksOnly`: their
/// whole-DXT textures never decoded, `MaterialLoader::blocks_only`) give
/// the same textures, field by field, and the same bytes as a dressing
/// decoded and then kept as blocks (`MapTexture::keep_blocks_only`), the
/// way the session cache kept them before. Prints each one's instructions
/// (this thread and those it starts) for docs/performance.md, "Guest
/// textures kept as blocks without decoding": each way twice, the second
/// counted (the first warms the OS's cache and the game's per-process
/// memos).
#[test]
fn a_kept_dressing_skips_decoding_and_keeps_the_same_textures() {
    use chimera_core::metrics::counters::InstructionCounter;
    use chimera_world::map::BlocksOnly;
    let guests: Vec<_> = guests().into_iter().filter(|(g, _)| *g != combat_arms::game::GAME).collect();
    if guests.is_empty() {
        eprintln!("skipping: needs an install of CS:S, HL2:DM or TF2");
        return;
    }
    for (game, dress) in guests {
        let run = |blocks: bool| {
            let mut counter = InstructionCounter::current_with_children();
            let at = counter.read().unwrap_or(0);
            let mut m = MapData::default();
            if blocks {
                m.memos.insert(BlocksOnly);
            }
            dress(&mut m);
            m.memos.remove::<BlocksOnly>();
            for t in &mut m.textures {
                t.keep_blocks_only();
            }
            (m, counter.read().unwrap_or(0).saturating_sub(at))
        };
        let _ = (run(false), run(true));
        let (decoded, before) = run(false);
        let (blocks, after) = run(true);
        assert_eq!(decoded.textures.len(), blocks.textures.len(), "{game}: as many textures");
        let mut kept_blocks = 0;
        for (a, b) in decoded.textures.iter().zip(&blocks.textures) {
            let same = a.name == b.name
                && a.srgb == b.srgb
                && (a.width, a.height) == (b.width, b.height)
                && a.rgba8 == b.rgba8
                && a.mips == b.mips
                && a.bc == b.bc;
            assert!(same, "{game}: texture {} differs", a.name);
            kept_blocks += usize::from(b.blocks_only());
        }
        assert!(kept_blocks > 0, "{game}: some textures are whole DXT blocks");
        let (x, y) = (postcard::to_allocvec(&decoded).unwrap(), postcard::to_allocvec(&blocks).unwrap());
        assert!(x == y, "{game}: the dressings encode differently");
        eprintln!(
            "{game}: {} textures ({kept_blocks} blocks only); decoded then kept {:.0} M instructions, kept without decoding {:.0} M ({:.0}% fewer)",
            blocks.textures.len(),
            before as f64 / 1e6,
            after as f64 / 1e6,
            100.0 * (1.0 - after as f64 / before.max(1) as f64)
        );
    }
}
