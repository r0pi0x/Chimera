//! The opt-in swap effect (`client::swap_effect`): off by default
//! (`party_swap_effect 0`), nothing on a swap; on, the local player's
//! identity changing flashes once per swap, another character's gets one
//! puff, and a first identity, an unchanged one or a dead character's
//! shows nothing.

use bevy::prelude::*;
use chimera_client::client::swap_effect::{FLASH_SECONDS, SwapEffectPlugin, SwapEffectSetting, SwapEffectView};
use chimera_core::{
    core::{Dead, LocalPlayer},
    slots::GameIdentity,
};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

fn world(on: bool) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, SwapEffectPlugin));
    });
    if on {
        sim.app.world_mut().resource_mut::<SwapEffectSetting>().on = 1;
    }
    let w = sim.app.world_mut();
    let me = w.spawn((LocalPlayer, GameIdentity("cs_source".into()), Transform::default(), GlobalTransform::default())).id();
    let other = w.spawn((GameIdentity("cs_source".into()), Transform::default(), GlobalTransform::default())).id();
    sim.ticks(2);
    (sim, me, other)
}

fn set(sim: &mut Sim, e: Entity, game: &str) {
    sim.app.world_mut().entity_mut(e).insert(GameIdentity(game.into()));
    sim.ticks(1);
}

fn view(sim: &Sim) -> SwapEffectView {
    *sim.app.world().resource::<SwapEffectView>()
}

#[test]
fn off_by_default_nothing_shows() {
    let (mut sim, me, other) = world(false);
    assert_eq!(sim.app.world().resource::<SwapEffectSetting>().on, 0);
    set(&mut sim, me, "combat_arms");
    set(&mut sim, other, "combat_arms");
    let v = view(&sim);
    assert_eq!((v.swaps, v.puffs), (0, 0));
    assert_eq!(v.flash, 0.0);
}

#[test]
fn on_once_per_swap() {
    let (mut sim, me, other) = world(true);
    let v = view(&sim);
    assert_eq!((v.swaps, v.puffs), (0, 0), "a first identity is no swap");
    set(&mut sim, me, "combat_arms");
    let v = view(&sim);
    assert_eq!(v.swaps, 1);
    assert!(v.flash > 0.0 && v.flash <= FLASH_SECONDS, "flashing: {}", v.flash);
    // The same identity again (a replicated rewrite) is no swap.
    set(&mut sim, me, "combat_arms");
    assert_eq!(view(&sim).swaps, 1);
    // The flash fades.
    sim.seconds(f64::from(FLASH_SECONDS) + 0.1);
    assert_eq!(view(&sim).flash, 0.0);
    set(&mut sim, me, "cs_source");
    assert_eq!(view(&sim).swaps, 2);
    // Another character: a puff, no flash.
    sim.seconds(0.5);
    set(&mut sim, other, "combat_arms");
    let v = view(&sim);
    assert_eq!((v.swaps, v.puffs), (2, 1));
    assert_eq!(v.flash, 0.0);
    // A dead character's change shows nothing.
    sim.app.world_mut().entity_mut(other).insert(Dead { since: 0.0 });
    set(&mut sim, other, "cs_source");
    assert_eq!(view(&sim).puffs, 1);
}

/// With the effect on, the local player's swap sounds the cue packs'
/// `swap` cue once (the new game's pack's, else the first that has one:
/// a pack without one, as this test's Combat Arms, takes the first's);
/// another's swap, the effect off, or a definition's `cues = "none"`
/// sounds nothing.
#[test]
fn a_local_swap_sounds_the_swap_cue_once() {
    use chimera_client::client::{
        callouts::{CalloutView, CalloutsPlugin},
        swap_effect::{SWAP_CUE, swap_cue},
    };
    use chimera_core::cues::{CueAppExt, CuePacks};
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, SwapEffectPlugin, CalloutsPlugin));
        app.register_cues("id = \"cs\"\nname = \"CS\"\ngame = \"cs_source\"\n[sounds]\nswap = \"sound/swap.wav\"\n");
        app.register_cues("id = \"ca\"\nname = \"CA\"\ngame = \"combat_arms\"\n[sounds]\nrung_up = \"up.wav\"\n");
    });
    sim.app.world_mut().resource_mut::<SwapEffectSetting>().on = 1;
    let w = sim.app.world_mut();
    let me = w.spawn((LocalPlayer, GameIdentity("cs_source".into()), Transform::default(), GlobalTransform::default())).id();
    let other = w.spawn((GameIdentity("cs_source".into()), Transform::default(), GlobalTransform::default())).id();
    sim.ticks(2);
    let heard = |sim: &Sim| sim.app.world().resource::<CalloutView>().heard.iter().filter(|h| *h == SWAP_CUE).count();
    assert_eq!(heard(&sim), 0, "a first identity is no swap");
    set(&mut sim, other, "combat_arms");
    assert_eq!(heard(&sim), 0, "another's swap: its puff, no sound");
    set(&mut sim, me, "combat_arms");
    assert_eq!(heard(&sim), 1, "the local swap");
    set(&mut sim, me, "combat_arms");
    assert_eq!(heard(&sim), 1, "no swap, no sound");
    set(&mut sim, me, "cs_source");
    assert_eq!(heard(&sim), 2);
    let packs = sim.app.world().resource::<CuePacks>().clone();
    assert_eq!(swap_cue(&packs, None, "combat_arms").map(|p| p.id.as_str()), Some("cs"), "the first pack with one");
    assert_eq!(swap_cue(&packs, None, "cs_source").map(|p| p.id.as_str()), Some("cs"));
    assert!(swap_cue(&packs, Some("none"), "cs_source").is_none(), "cues = \"none\"");
    // Off: nothing.
    sim.app.world_mut().resource_mut::<SwapEffectSetting>().on = 0;
    set(&mut sim, me, "combat_arms");
    assert_eq!(heard(&sim), 2);
}
