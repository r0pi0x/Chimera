//! The Source guests' dressings kept for the session
//! (`source_engine::content::guest`): a second map load of the app copies
//! them, converting nothing again, and every texture index the copied
//! parts hold names the texture a fresh dressing of the same map gives;
//! a guest the match can't deal (`MapLoadOptions::dealt`) isn't dressed.
//! Passes with a note without the CS:S, HL2:DM and Combat Arms installs.

use std::{collections::BTreeSet, sync::Arc};

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::{
    LoadContext, MapData, MapLoadOptions,
    texture_refs::{Ref, TextureRefs},
};
use source_engine::content::guest;

fn installs() -> bool {
    let config = LocalConfig::load().ok();
    config.is_some_and(|c| {
        c.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
            .is_some_and(|p| chimera_core::mount::served::usable("cs_source", &p, "cstrike"))
            && c.game_path(combat_arms::game::GAME).is_some_and(|p| chimera_core::mount::served::usable("combat_arms", &p, "Game"))
    }) && hl2dm::game::INFO.install().is_some()
}

/// Each texture index of `map`'s parts that draw, as the name and size of
/// the texture it points at (cubemaps by name), in the parts' order.
fn named_refs(map: &MapData) -> Vec<String> {
    let mut kit = map.kit.clone();
    let mut particles = map.particles.materials.clone();
    let mut anims = map.texture_anims.clone();
    let mut out = Vec::new();
    let mut name = |r: Ref, i: &mut usize| {
        out.push(match r {
            Ref::Texture => map.textures.get(*i).map_or(format!("missing texture {i}"), |t| format!("{} {}x{}", t.name, t.width, t.height)),
            Ref::Cubemap => map.cubemaps.get(*i).map_or(format!("missing cubemap {i}"), |c| c.name.clone()),
        })
    };
    kit.texture_refs(&mut name);
    particles.iter_mut().for_each(|p| p.texture_refs(&mut name));
    anims.iter_mut().for_each(|a| a.texture_refs(&mut name));
    out
}

fn kit_of(id: &str, options: &MapLoadOptions) -> MapData {
    party_kit::load_map_with(id, options).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn a_second_load_copies_the_guests_and_their_indices_hold() {
    if !installs() {
        eprintln!("skipping: needs the CS:S, HL2:DM and Combat Arms installs");
        return;
    }
    let context = LoadContext::default();
    let options = MapLoadOptions::default().in_context(&context);
    // The kit with no guest (none dealt): what each guest is added to.
    let bare = kit_of("chimera:greybox", &MapLoadOptions { dealt: Some(Arc::new(BTreeSet::new())), ..options.clone() });
    assert!(!bare.textures.is_empty(), "Combat Arms' parts first: the guests' indices move past them");
    assert!(bare.kit.characters.iter().all(|c| c.game == Some("combat_arms")), "no guest dressed");

    // Fresh: each guest's own dressing on the map.
    let mut fresh = bare.clone();
    cs_source::game::dress_guest_with(&mut fresh, cs_source::game::BUNDLE.kept, false).unwrap();
    hl2dm::game::maps::dress_guest_with(&mut fresh, hl2dm::game::BUNDLE.kept, false).unwrap();

    // Kept: converted on the first load, copied on the second.
    let mut kept = bare.clone();
    for _ in 0..2 {
        kept = bare.clone();
        for (game, dress) in [
            (cs_source::game::GAME, cs_source::game::bundle::GUEST.dress),
            (hl2dm::game::GAME, hl2dm::game::GUEST.dress),
        ] {
            dress(&mut kept, &options).unwrap_or_else(|e| panic!("{game}: {e}"));
        }
    }
    for game in [cs_source::game::GAME, hl2dm::game::GAME] {
        assert_eq!(guest::conversions(&context, game, true), 1, "{game}: converted once for two loads");
    }
    let bodies = |m: &MapData| m.kit.characters.iter().map(|c| (c.game, c.name.clone(), c.team)).collect::<Vec<_>>();
    assert_eq!(bodies(&fresh), bodies(&kept));
    let keys = |m: &MapData| (m.kit.held.iter().map(|h| h.key.clone()).collect::<Vec<_>>(), m.kit.view_models.iter().map(|v| v.key.clone()).collect::<Vec<_>>());
    assert_eq!(keys(&fresh), keys(&kept));
    let (a, b) = (named_refs(&fresh), named_refs(&kept));
    assert_eq!(a.len(), b.len(), "as many texture indices");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x, y, "index {i} of the parts' texture indices");
    }
    assert!(b.iter().all(|n| !n.starts_with("missing")), "every index in range");
    let entries = |m: &MapData| m.sounds.entries.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(entries(&fresh), entries(&kept), "the same sound entries");
    let sound_waves = |m: &MapData, e: &str| {
        m.sounds.entries[e].waves.iter().map(|w| m.sounds.clips[*w].samples.len()).collect::<Vec<_>>()
    };
    for e in fresh.sounds.entries.keys() {
        assert_eq!(sound_waves(&fresh, e), sound_waves(&kept, e), "{e}'s clips");
    }
    let huds = |m: &MapData| m.kit.guest_huds.iter().map(|h| h.game.clone()).chain(m.kit.hud.iter().map(|h| h.game.clone())).collect::<Vec<_>>();
    assert_eq!(huds(&fresh), huds(&kept));
    assert_eq!(fresh.kit.decals.decals.len(), kept.kit.decals.decals.len());
    assert_eq!(fresh.kit.shells.len(), kept.kit.shells.len());

    // A match that deals CS:S alone: HL2:DM isn't dressed.
    let cs_only = MapLoadOptions { dealt: Some(Arc::new(BTreeSet::from([cs_source::game::GAME.to_string()]))), ..options.clone() };
    let only = kit_of("chimera:greybox", &cs_only);
    let games: BTreeSet<_> = only.kit.characters.iter().filter_map(|c| c.game).collect();
    assert!(games.contains(cs_source::game::GAME) && !games.contains(hl2dm::game::GAME), "{games:?}");
}

/// Each Source guest's dressing of the code-built maps' kit (Combat Arms'
/// parts on it), as before the cache (its own dressing, converting every
/// time) and kept (the first load converts, the next copy): wall time and
/// this thread's instructions (`work::now`; the texture decoding's
/// threads aside), printed for docs/performance.md ("Guest dressings kept
/// for the session").
#[test]
fn the_guests_cost_cold_and_warm() {
    if !installs() {
        eprintln!("skipping: needs the CS:S, HL2:DM and Combat Arms installs");
        return;
    }
    let context = LoadContext::default();
    let options = MapLoadOptions::default().in_context(&context);
    let bare = kit_of("chimera:greybox", &MapLoadOptions { dealt: Some(Arc::new(BTreeSet::new())), ..options.clone() });
    let measure = |what: &str, dress: &dyn Fn(&mut MapData)| {
        let mut map = bare.clone();
        let (t, i) = (std::time::Instant::now(), chimera_core::metrics::work::now());
        dress(&mut map);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let m = i.zip(chimera_core::metrics::work::now()).map_or(0.0, |(a, b)| (b - a) as f64 / 1e6);
        eprintln!("{what}: {ms:.0} ms, {m:.0} M instructions, {} textures added", map.textures.len() - bare.textures.len());
    };
    for i in 0..2 {
        measure(&format!("CS:S converting, load {i}"), &|m| cs_source::game::dress_guest_with(m, cs_source::game::BUNDLE.kept, false).unwrap());
        measure(&format!("HL2:DM converting, load {i}"), &|m| hl2dm::game::maps::dress_guest_with(m, hl2dm::game::BUNDLE.kept, false).unwrap());
    }
    for i in 0..3 {
        measure(&format!("CS:S kept, load {i}"), &|m| (cs_source::game::bundle::GUEST.dress)(m, &options).unwrap());
        measure(&format!("HL2:DM kept, load {i}"), &|m| (hl2dm::game::GUEST.dress)(m, &options).unwrap());
    }
}
